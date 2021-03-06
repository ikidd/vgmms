use vgtk::ext::*;
use vgtk::lib::gtk::{self, *, Box as GtkBox};
use vgtk::lib::glib;
use vgtk::lib::gio::{ActionExt, ApplicationFlags, SimpleAction};
use vgtk::{gtk, Component, UpdateAction, VNode};

use std::default::Default;
use std::sync::{Arc, RwLock};
use std::boxed::Box;

use crate::types::*;

use crate::{chat_log, file_chooser, new_chat, select_chat};
use crate::{db, dbus, once};

#[derive(Clone, Default)]
pub struct WindowModel {
	state: Arc<RwLock<VgmmsState>>,
	current_page: i32,
}

#[derive(Clone, Debug)]
pub enum UiMessage {
	Notif(dbus::DbusNotification),
	Send((Chat, Vec<DraftItem>)),
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

fn message_id_from_hex(s: &str) -> MessageId {
	use std::io::Write;
	let mut id = [0u8; 20];
	if let Ok(bytes) = hex::decode(s) {
		let _ = (&mut id[..]).write(&*bytes);
	}
	id
}

fn apply_tab_label(nb: &Notebook, child: &Widget)
{
	let text = child.get_widget_name();
	if text != "" {
		let label = Label::new(Some(&*text));
		label.set_width_chars(12);
		label.set_ellipsize(pango::EllipsizeMode::End);
		label.set_tooltip_text(Some(&*text));
		nb.set_tab_label(child, Some(&label));
	}
}

impl Component for WindowModel {
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
			Send((chat, draft_items)) => {
				if draft_items.len() == 0 {
					return UpdateAction::None
				}
				let mut state = self.state.write().unwrap();
				state.send_message(&chat, draft_items);
				UpdateAction::Render
			},
			AskDelete(_msg_id) => {
				//
				UpdateAction::None
			},
			Delete(msg_id) => {
				let mut state = self.state.write().unwrap();
				if let Err(e) = db::delete_message(&mut state.db_conn, &msg_id) {
					eprintln!("error deleting message: {}", e);
				}
				state.messages.remove(&msg_id);
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
				let state = self.state.read().unwrap();

				let fut = vgtk::run_dialog_props::<select_chat::SelectChatDialog>(vgtk::current_window().as_ref(),
					select_chat::SelectChatDialog {
						my_number: state.my_number,
						chats_summaries: state.summarize_all(),
						numbers_shared: numbers_shared.clone(),
						on_new_chat: {let cb: vgtk::Callback<()> = Box::new(once::once(move |()| {
							use glib::object::Cast;
							let w = vgtk::current_object().unwrap();
							let w = w.downcast_ref::<Widget>().unwrap();
							let dialog = w.get_toplevel().unwrap();
							let dialog = dialog.downcast_ref::<Dialog>().unwrap();
							dialog.response(ResponseType::Other(0));
						})).into(); cb},
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
				let state = self.state.read().unwrap();

				let fut = vgtk::run_dialog_props::<new_chat::NewChat>(vgtk::current_window().as_ref(),
					new_chat::NewChat {
						my_number: state.my_number,
						my_country: Some(state.my_country),
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
				let state = self.state.read().unwrap();
				let (notify, path_result) = futures::channel::oneshot::channel();

				let att = match state.attachments.get(&att_id) {
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
							if let Err(e) = att.clone().with_data(|data| {
								if let Err(e) = std::fs::write(&path, data) {
									eprintln!("error saving attachment: {}", e);
								}
							}) {
								eprintln!("error loading attachment data to save it: {}", e);
							}
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

	fn view(&self) -> VNode<WindowModel> {
		let state = self.state.read().unwrap();
		let my_number = state.my_number;
		let my_country = state.my_country;
		let no_chats = state.chats.len() == 0;
		let no_chats_open = state.open_chats.len() == 0;
		let actions = vec![
			gtk! {<SimpleAction::new("save-attachment-dialog",
				Some(glib::VariantTy::new("t").unwrap())) enabled=true
				on activate=|_a, id| UiMessage::SaveAttachmentDialog(id.unwrap().get().unwrap())
			/>},
			gtk! {<SimpleAction::new("delete-message",
				Some(glib::VariantTy::new("s").unwrap())) enabled=true
				on activate=|_a, id| UiMessage::Delete(message_id_from_hex(&id.unwrap().get::<String>().unwrap()))
			/>},
			gtk! {<SimpleAction::new("exit", None) Application::accels=["<Ctrl>q"].as_ref() enabled=true
				on activate=|_a, _| UiMessage::Exit
			/>},
			gtk! {<SimpleAction::new("new-tab", None) Application::accels=["<Ctrl>t"].as_ref() enabled=true
				on activate=|_a, _| UiMessage::SelectChat
			/>},
			gtk! {<SimpleAction::new("close-tab", None) Application::accels=["<Ctrl>w"].as_ref() enabled=true
				on activate=|_a, _| UiMessage::CloseCurrentChat
			/>},
			gtk! {<SimpleAction::new("open-chat",
				/* the glib crate has not yet released a version with array variant support */
				Some(glib::VariantTy::new("s"/*"as"*/).unwrap())) enabled=true
				on activate=|_a, num_strs| {
					/*let num_strs = num_strs.unwrap().get::<Vec<String>>();*/
					let nums_str = num_strs.unwrap().get::<String>().unwrap();
					let mut valid = true;
					let mut nums = vec![];
					for num_str in nums_str.split(',') {
						if let Some(n) = Number::normalize(num_str, my_country) {
							nums.push(n);
						} else {
							eprintln!("could not parse number '{}'", num_str);
							valid = false;
						}
					}
					if valid {
						UiMessage::OpenChat(nums)
					} else {
						UiMessage::Nop
					}
				}
			/>},
		].into_iter();
		gtk! {
			<Application::new_unwrap(Some("org.vgmms"), ApplicationFlags::REPLACE)>
				{actions}
				<ApplicationWindow default_width=180 default_height=300 border_width=5
					on realize=|w| {
						w.connect_delete_event(|w, _ev| { w.hide(); glib::signal::Inhibit(true) });
						UiMessage::Nop
					}
				>
					<GtkBox::new(Orientation::Vertical, 0)>{
						if no_chats { gtk! {
							<Button::from_icon_name(Some("list-add"), IconSize::Button)
								GtkBox::expand=true valign=Align::Center
								label="Start new chat"
								on clicked=|_| UiMessage::DefineChat
							/>
						} } else if no_chats_open { gtk! {
							<@select_chat::SelectChat
								my_number=my_number
								chats_summaries=state.summarize_all()
								on select=|nums| UiMessage::OpenChat(nums)
								on new_chat=|_| UiMessage::DefineChat
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
									<Button::from_icon_name(Some("window-close"), IconSize::Menu)
										relief=ReliefStyle::None
										on clicked=|_| UiMessage::CloseCurrentChat
									/>
									<Button::from_icon_name(Some("list-add"), IconSize::Menu)
										relief=ReliefStyle::None
										on clicked=|_| {if no_chats { UiMessage::DefineChat } else { UiMessage::SelectChat }}
									/>
								</GtkBox>
								{
									state.open_chats.iter().map(move |c| gtk! {
										<EventBox Notebook::tab_expand=true
											Notebook::tab_label=c.get_name(&my_number)
											widget_name=c.get_name(&my_number)>
											<@chat_log::ChatLog
												chat=c
												state=self.state.clone()
												on send=|c_drafts| UiMessage::Send(c_drafts)
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
