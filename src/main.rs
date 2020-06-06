#![recursion_limit="1024"]

#[macro_use]
extern crate lazy_static;

use vgtk::ext::*;
use vgtk::lib::gio::{self, ApplicationFlags};
use vgtk::lib::gtk::{*, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode};

use std::default::Default;
use std::sync::{Arc, RwLock};
use std::boxed::Box;

/* widgets */
mod chat;
mod input_box;
mod new_chat;

/* logic */
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
	DefineChat,
	OpenChat(Vec<Number>),
	Nop,
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
				if self.current_page >= 0 {
					let mut state = self.state.write().unwrap();
					let chat = state.open_chats.remove(self.current_page as usize);
					if let Err(e) = db::close_chat(&mut state.db_conn, &chat) {
						eprintln!("error saving chat state to database: {}", e);
					}
					if self.current_page >= state.open_chats.len() as i32 {
						self.current_page -= 1;
					}
				}
				UpdateAction::Render
			},
			/*CloseChat(nums) => {
				//close tab and save to db
			},*/
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
				match state.chats.iter().enumerate().find(|&(_i, c)| c.0.numbers == nums) {
					Some((_idx, c)) => {
						println!("found chat {:?}", c);
						/*TODO: switch to it*/
					},
					None => {
						//if it doesn't, create it and save to db
						nums.push(my_number);
						nums.sort();
						let chat = Chat{ numbers: nums };

						println!("saving chat {:?}", chat);
						if let Err(e) = db::open_chat(&mut state.db_conn, &chat, self.current_page) {
							eprintln!("error saving chat state to database: {}", e);
						}
						if self.current_page < 0 {
							self.current_page = 0
						}
						state.open_chats.insert(self.current_page as usize, chat.clone());
						state.chats.insert(chat, None);
					},
				}
				UpdateAction::Render
			},
			Nop => {
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<Model> {
		let state = self.state.read().unwrap();
		let my_number = state.my_number;
		let no_chats_open = state.open_chats.len() == 0;
		gtk! {
			<Application::new_unwrap(Some("org.vgmms"), ApplicationFlags::empty())>
				<Window default_width=180 default_height=300 border_width=5 on destroy=|_| UiMessage::Exit>
					<GtkBox::new(Orientation::Vertical, 0)>{
						if no_chats_open { gtk! {
							<Button::new_from_icon_name(Some("list-add"), IconSize::Button)
								GtkBox::expand=true valign=Align::Center
								label="Start new chat"
								on clicked=|_| UiMessage::DefineChat
							/>
						} } else { gtk!{
							<Notebook GtkBox::expand=true scrollable=true
								property_page=self.current_page
								on switch_page=|_nb, _pg, n| UiMessage::ChatChanged(n as i32)>
								<GtkBox::new(Orientation::Horizontal, 0)
									Notebook::action_widget_end=true>
									<Button::new_from_icon_name(Some("window-close"), IconSize::Menu)
										relief=ReliefStyle::None
										on clicked=|_| UiMessage::CloseCurrentChat
									/>
									<Button::new_from_icon_name(Some("list-add"), IconSize::Menu)
										relief=ReliefStyle::None
										on clicked=|_| UiMessage::DefineChat
									/>
								</GtkBox>
								{
									self.state.read().unwrap().open_chats.iter().map(move |c| gtk! {
										<EventBox Notebook::tab_label=c.get_name(&my_number)>
											<@ChatModel
												chat=c
												state=self.state.clone()
											/>
										</EventBox>})
								}
							</Notebook>
						}}
					}</GtkBox>
				</Window>
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
