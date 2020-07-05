#![recursion_limit="1024"]

#[macro_use]
extern crate lazy_static;

use vgtk::ext::*;
use vgtk::lib::gio::{self, ActionExt, ApplicationFlags, SimpleAction};
use vgtk::lib::gtk::{*, Box as GtkBox};
use vgtk::lib::glib;
use vgtk::{gtk, Component, UpdateAction, VNode};

use std::default::Default;
use std::sync::{Arc, RwLock};
use std::boxed::Box;

/* widgets */
mod chat;
mod file_chooser;
mod input_box;
mod new_chat;
mod select_chat;

/* logic */
mod new_custom;
mod once;
mod types;
mod smil;
mod state;

/* persistence */
mod db;

/* dbus interfaces */
mod dbus;
mod mmsd_manager;
mod mmsd_service;
mod ofono_manager;
mod ofono_simmanager;

use chat::*;
use types::*;

#[derive(Clone, Default)]
struct Model {
	state: Arc<RwLock<VgmmsState>>,
	current_page: i32,
}

#[derive(Clone, Debug)]
enum UiMessage {
	Notif(dbus::DbusNotification),
	Send(Vec<MessageItem>, Chat),
	AskDelete(MessageId),
	Delete(MessageId),
	Exit,
	ChatChanged(i32),
	CloseCurrentChat,
	SelectChat,
	DefineChat,
	OpenChat(Vec<Number>),
	SaveAttachmentDialog(AttachmentId),
	Nop,
}

fn apply_tab_label(nb: &Notebook, child: &Widget)
{
	if let Some(text) = child.get_widget_name() {
		let label = Label::new(Some(&*text));
		label.set_width_chars(12);
		label.set_ellipsize(pango::EllipsizeMode::End);
		label.set_tooltip_text(Some(&*text));
		nb.set_tab_label(child, Some(&label));
	}
}

impl Component for Model {
	type Message = UiMessage;
	type Properties = ();

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		use UiMessage::*;
		match msg {
			Notif(notif) => {
				let mut state = self.state.write().unwrap();
				state.handle_notif(notif);
				UpdateAction::Render
			},
			Send(_mi, _chat) => {
				UpdateAction::Render
			},
			AskDelete(_msg_id) => {
				//
				UpdateAction::None
			},
			Delete(_msg_id) => {
				//messages.
				UpdateAction::Render
			},
			Exit => {
				vgtk::quit();
				UpdateAction::None
			},
			ChatChanged(n) => {
				self.current_page = n;
				UpdateAction::None
			},
			CloseCurrentChat => {
				let mut state = self.state.write().unwrap();
				if state.open_chats.len() == 0 {
					self.current_page = -1;
				}
				if self.current_page >= 0 {
					let chat = state.open_chats.remove(self.current_page as usize);
					if let Err(e) = db::close_chat(&mut state.db_conn, &chat) {
						eprintln!("error saving chat state: {}", e);
					}
					if self.current_page >= state.open_chats.len() as i32 {
						self.current_page -= 1;
					}
					UpdateAction::Render
				} else {
					UpdateAction::None
				}
			},
			/*CloseChat(nums) => {
				//close tab and save to db
			},*/
			SelectChat => {
				use std::sync::Mutex;
				let numbers_shared: Arc<Mutex<Vec<Number>>> = Default::default();

				let fut = vgtk::run_dialog_props::<select_chat::SelectChatDialog>(vgtk::current_window().as_ref(),
					select_chat::SelectChatDialog {
						state: self.state.clone(),
						numbers_shared: numbers_shared.clone(),
						numbers: vec![],
					});

				let fut = async move {
					match fut.await {
						Ok(ResponseType::Other(0)) => {
							DefineChat
						},
						Ok(ResponseType::Accept) => {
							let nums = numbers_shared.lock().unwrap();
							if nums.len() > 0 {
								OpenChat(nums.clone())
							} else {
								Nop
							}
						},
						_ => Nop,
					}
				};

				UpdateAction::Defer(Box::pin(fut))
			},
			DefineChat => {
				use std::sync::Mutex;
				let numbers_shared: Arc<Mutex<Vec<Number>>> = Default::default();

				let fut = vgtk::run_dialog_props::<new_chat::NewChat>(vgtk::current_window().as_ref(),
					new_chat::NewChat {
						my_number: self.state.read().unwrap().my_number,
						my_country: Some(self.state.read().unwrap().my_country),
						numbers: vec![],
						partial_num: String::new(),
						numbers_shared: numbers_shared.clone(),
					});

				let fut = async move {
					if let Ok(ResponseType::Accept) = fut.await {
						OpenChat(numbers_shared.lock().unwrap().clone())
					} else {
						Nop
					}
				};

				UpdateAction::Defer(Box::pin(fut))
			},
			OpenChat(mut nums) => {
				let mut state = self.state.write().unwrap();
				let my_number = state.my_number;

				if !nums.contains(&my_number) {
					nums.push(my_number);
				}
				nums.sort();

				/* bail if nums is trivial */
				if nums.len() == 1 {
					return UpdateAction::None;
				}

				if state.open_chats.len() == 0 {
					self.current_page = -1;
				}
				match state.open_chats.iter().enumerate().find(|&(_i, c)| c.numbers == nums) {
					Some((idx, _c)) => {
						self.current_page = idx as i32;
					},
					None => {
						self.current_page += 1;
						let chat = Chat { numbers: nums };

						if state.chats.get(&chat).is_some() {
							/* if chat exists but isn't open, set its tab */
							if let Err(e) = db::set_chat_tab(&mut state.db_conn, &chat, self.current_page) {
								eprintln!("error saving chat state: {}", e);
							}
						} else {
							/* if it doesn't, create it and save to db */
							if let Err(e) = db::insert_chat(&mut state.db_conn, &chat, self.current_page, None) {
								eprintln!("error saving chat: {}", e);
							}
							state.chats.insert(chat.clone(), None);
						}
						state.open_chats.insert(self.current_page as usize, chat);
					},
				}
				UpdateAction::Render
			},
			SaveAttachmentDialog(att_id) => {
				let (notify, path_result) = futures::channel::oneshot::channel();

				let att = match self.state.read().unwrap().attachments.get(&att_id) {
					Some(att) => att.clone(),
					None => {
						eprintln!("attachment not found!");
						return UpdateAction::None
					},
				};
				let filename = att.name.to_str().unwrap_or("");

				let fut = vgtk::run_dialog_props::<file_chooser::FileChooser>(vgtk::current_window().as_ref(),
					file_chooser::FileChooser {
						on_choose: {let cb: vgtk::Callback<Vec<std::path::PathBuf>> = Box::new(once::once(move |filenames| {
							let _ = notify.send(filenames);
						})).into(); cb},
						action: Some(FileChooserAction::Save),
						title: "Save attachment".into(),
						select_multiple: false,
						accept_label: "_Save".into(),
						default_name: Some(filename.to_owned()),
					});

				let fut = async move {
					if let Ok(ResponseType::Accept) = fut.await {
						if let [path] = &*path_result.await.unwrap() {
							att.clone().with_data(|data| {
								std::fs::write(&path, data)
							});
						}
					}
					Nop
				};

				UpdateAction::Defer(Box::pin(fut))
			},
			Nop => {
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<Model> {
		let state = self.state.read().unwrap();
		let my_number = state.my_number;
		let no_chats = state.chats.len() == 0;
		let no_chats_open = state.open_chats.len() == 0;
		gtk! {
			<Application::new_unwrap(Some("org.vgmms"), ApplicationFlags::empty())>
				<SimpleAction::new("save-attachment-dialog",
					Some(glib::VariantTy::new("t").unwrap())) enabled=true
					on activate=|_a, id| UiMessage::SaveAttachmentDialog(id.unwrap().get().unwrap()) />
				<SimpleAction::new("exit", None) Application::accels=["<Ctrl>q"].as_ref() enabled=true
					on activate=|_a, _| UiMessage::Exit />
				<SimpleAction::new("new-tab", None) Application::accels=["<Ctrl>t"].as_ref() enabled=true
					on activate=|_a, _| UiMessage::SelectChat />
				<SimpleAction::new("close-tab", None) Application::accels=["<Ctrl>w"].as_ref() enabled=true
					on activate=|_a, _| UiMessage::CloseCurrentChat />
				<ApplicationWindow default_width=180 default_height=300 border_width=5 on destroy=|_| UiMessage::Exit>
					<GtkBox::new(Orientation::Vertical, 0)>{
						if no_chats { gtk! {
							<Button::new_from_icon_name(Some("list-add"), IconSize::Button)
								GtkBox::expand=true valign=Align::Center
								label="Start new chat"
								on clicked=|_| UiMessage::DefineChat
							/>
						} } else if no_chats_open { gtk! {
							<@select_chat::SelectChat
								state=self.state.clone()
								on select=|nums| UiMessage::OpenChat(nums)
							/>
						} } else { use gtk::prelude::NotebookExtManual; gtk!{
							<Notebook GtkBox::expand=true scrollable=true
								property_page=self.current_page
								on switch_page=|_nb, _pg, n| UiMessage::ChatChanged(n as i32)
								on page_removed=|nb, child, n| {
									if let Some(ref prev) = nb.get_nth_page(Some(n-1)) {
										apply_tab_label(nb, prev);
									}
									UiMessage::Nop
								}
								on page_added=|nb, child, n| {
									apply_tab_label(nb, child);
									if let Some(ref next) = nb.get_nth_page(Some(n+1)) {
										apply_tab_label(nb, next);
									}
									UiMessage::Nop
								}>
								<GtkBox::new(Orientation::Horizontal, 0)
									Notebook::action_widget_end=true>
									<Button::new_from_icon_name(Some("window-close"), IconSize::Menu)
										relief=ReliefStyle::None
										on clicked=|_| UiMessage::CloseCurrentChat
									/>
									<Button::new_from_icon_name(Some("list-add"), IconSize::Menu)
										relief=ReliefStyle::None
										on clicked=|_| {if no_chats { UiMessage::DefineChat } else { UiMessage::SelectChat }}
									/>
								</GtkBox>
								{
									self.state.read().unwrap().open_chats.iter().map(move |c| gtk! {
										<EventBox Notebook::tab_expand=true
											Notebook::tab_label=c.get_name(&my_number)
											widget_name=c.get_name(&my_number)>
											<@ChatModel
												chat=c
												state=self.state.clone()
											/>
										</EventBox>})
								}
							</Notebook>
						}}
					}</GtkBox>
				</ApplicationWindow>
			</Application>
		}
	}
}

fn main() {
	use gio::prelude::ApplicationExtManual;
	use futures::stream::StreamExt;

	let notif_stream = dbus::start_recv();
	pretty_env_logger::init();
	let (app, scope) = vgtk::start::<Model>();
	std::thread::spawn(
		move || futures::executor::block_on(
			notif_stream.for_each(move |notif| {
				println!("notif sent!");
				scope.try_send(UiMessage::Notif(notif)).unwrap();
				futures::future::ready(())
			}))
	);
	std::process::exit(app.run(&[]));
}
