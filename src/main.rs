#![recursion_limit="512"]

#[macro_use]
extern crate lazy_static;

use vgtk::ext::*;
use vgtk::lib::gio::{self, ApplicationFlags};
use vgtk::lib::gtk::{*, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode};

use std::default::Default;
use std::sync::{Arc, RwLock};
use std::boxed::Box;

use std::ffi::OsString;

/* widgets */
mod chat;
mod input_box;
mod new_chat;

/* logic */
mod types;
mod smil;

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
}

#[derive(Clone, Debug)]
enum UiMessage {
	Notif(dbus::DbusNotification),
	Send(Vec<MessageItem>, Chat),
	AskDelete(MessageId),
	Delete(MessageId),
	Exit,
	DefineChat,
	NewChat(Vec<Number>),
	Nop,
}

fn read_file_chunk(path: &std::path::Path, start: u64, len: u64) -> Result<Vec<u8>, std::io::Error> {
	use std::io::{Read, Seek, SeekFrom};

	let mut file = std::fs::File::open(path)?;
	file.seek(SeekFrom::Start(start))?;
	let mut out = vec![0; len as usize];
	file.read_exact(&mut out[..])?;
	Ok(out)
}

fn parse_date(date: &str) -> chrono::format::ParseResult<u64> {
	match chrono::DateTime::parse_from_rfc3339(&date) {
		Err(e) if e.to_string().contains("invalid") => {
			let mut date = date.to_owned();
			date.insert(date.len()-2, ':');
			chrono::DateTime::parse_from_rfc3339(&date)
		},
		x => x,
	}.map(|x| x.timestamp() as u64)
}

impl VgmmsState {
	pub fn next_message_id(&mut self) -> MessageId {
		let id = self.next_message_id;

		self.next_message_id.increment();

		id
	}

	pub fn next_attachment_id(&mut self) -> AttachmentId {
		let id = self.next_attachment_id;
		self.next_attachment_id += 1;
		id
	}

	pub fn handle_notif(&mut self, notif: dbus::DbusNotification) {
		use self::dbus::DbusNotification::*;
		match notif {
			MmsStatusUpdate {
				id, status,
			} => {
				if let Some(msg) = self.messages.get_mut(&id) {
					msg.status = status;
				} else {
					eprintln!("cannot find message {} to update status", hex::encode(&id[..]));
				}
			},
			MmsReceived {
				id, date, subject: _, sender,
				recipients, attachments,
				smil: _,
			} => {
				let time = match parse_date(&date) {
					Ok(d) => d,
					Err(e) => {
						eprintln!("cannot parse timestamp {}: {}", date, e);
						return
					},
				};
				let mut contents = vec![];
				let mut text = String::new();
				for att in attachments {
					if att.mime_type.starts_with("text/plain") {
						/* fall back to remembering its attachment if we fail to read text from MMS file */
						if let Ok(new_text) = read_file_chunk(&att.disk_path, att.start, att.len) {
							let read = String::from_utf8_lossy(&*new_text);
							text.push_str(&*read);
							continue;
						}
					}

					let id = self.next_attachment_id();
					let att = Attachment {
						name: OsString::from(att.name),
						mime_type: att.mime_type,
						data: (att.disk_path, att.start, att.len),
					};
					if let Err(e) = db::insert_attachment(&mut self.db_conn, &id, &att) {
						eprintln!("error saving attachment to database: {}", e);
					}
					self.attachments.insert(id, att);
					contents.push(MessageItem::Attachment(id));
				}
				contents.insert(0, MessageItem::Text(text));
				
				if let Some(sender) = Number::normalize(&*sender, self.my_country) {
					let mut chat: Vec<_> = recipients.iter().filter_map(|r| Number::normalize(&*r, self.my_country)).collect();
					chat.push(sender);
					chat.sort();
					let message = MessageInfo {
						sender,
						chat,
						time,
						contents,
						status: MessageStatus::Received,
					};
					println!("inserting mms {}: {:?}", hex::encode(&id[..]), message);
					if let Err(e) = db::insert_message(&mut self.db_conn, &id, &message) {
						eprintln!("error saving message to database: {}", e);
					}
					self.messages.insert(id, message);
				} else {
					eprintln!("cannot parse number {}", sender);
				}
			},
			SmsReceived {
				message, date, sender,
			} => {
				let time = match parse_date(&date) {
					Ok(d) => d,
					Err(e) => {
						eprintln!("cannot parse timestamp {}: {}", date, e);
						return
					},
				};
				if let Some(sender) = Number::normalize(&*sender, self.my_country) {
					let mut chat = vec![sender, self.my_number];
					chat.sort();
					let id = self.next_message_id();
					let message = MessageInfo {
						sender,
						chat,
						time,
						contents: vec![MessageItem::Text(message)],
						status: MessageStatus::Received,
					};
					println!("inserting sms {}: {:?}", hex::encode(&id[..]), message);
					if let Err(e) = db::insert_message(&mut self.db_conn, &id, &message) {
						eprintln!("error saving message to database: {}", e);
					}
					self.messages.insert(id, message);
				} else {
					eprintln!("cannot parse number {}", sender);
				}
			}
		}
	}
}

//fn ensure_chat_for(&mut self, recipients: ) -> 

use std::collections::{BTreeMap, HashMap};

impl Default for VgmmsState {
	fn default() -> Self {
		let mut conn = db::connect().unwrap();
		let _ = db::create_tables(&mut conn);

		let next_message_id = match db::get_next_message_id(&mut conn) {
			Ok(id) => id,
			_ => {
				let mut id = [0u8; 20]; id.increment(); id
			},
		};

		let next_attachment_id = match db::get_next_attachment_id(&mut conn) {
			Ok(id) => id,
			_ => 1,
		};

		let mut messages = BTreeMap::new();
		{
			let mut q = db::Query::new(&mut conn).unwrap();

			for res in db::get_all_messages(&mut q).unwrap().unwrap() {
				match res {
					Ok((id, m)) => {
						//println!("from db inserting {:?}", m);
						messages.insert(id, m);
					},
					Err(e) => {
						eprintln!("error loading messages from db: {}", e)
					},
				}
			}
		}

		let attachments = db::get_all_attachments(&mut conn).unwrap();

		let modem_path = match &*dbus::get_modem_paths().unwrap() {
			[m] => m.to_owned(),
			ms => panic!("expected 1 modem, got {}", ms.len()),
		};
		let my_number = dbus::get_my_number(&modem_path).unwrap()
			.expect("could not determine subscriber phone number");
		let my_country = Number::get_country(&my_number)
			.expect("could not determine country of subscriber phone number");
		let my_number = Number::normalize(&my_number, my_country)
			.expect("could not parse subscriber phone number");

		let mut chats = HashMap::new();
		let chats_vec = db::get_all_chats(&mut conn).unwrap();
		for c in chats_vec.into_iter() {
			chats.insert(c.numbers.clone(), c);
		}

		VgmmsState {
			chats,
			messages,
			contacts: Default::default(),
			attachments,
			next_message_id,
			next_attachment_id,
			my_number,
			my_country,
			modem_path,
			db_conn: conn,
		}
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
			DefineChat => {
				use std::sync::{Mutex};
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
						NewChat(numbers_shared.lock().unwrap().clone())
					} else {
						Nop
					}
				};

				UpdateAction::Defer(Box::pin(fut))
			},
			/*CloseChat(nums) => {
				//close tab and save to db
			},*/
			NewChat(mut nums) => {
				println!("newchat {:?}", nums);
				let mut state = self.state.write().unwrap();
				let my_number = state.my_number;
				match state.chats.iter().enumerate().find(|&(_i, c)| c.1.numbers == nums) {
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
						if let Err(e) = db::insert_chat(&mut state.db_conn, &chat) {
							eprintln!("error saving chat to database: {}", e);
						}
						state.chats.insert(chat.numbers.clone(), chat);
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
		let chats_empty = state.chats.len() == 0;
		gtk! {
			<Application::new_unwrap(Some("org.vgmms"), ApplicationFlags::empty())>
				<Window default_width=180 default_height=300 border_width=5 on destroy=|_| UiMessage::Exit>
					<GtkBox::new(Orientation::Vertical, 0)>{
						if chats_empty { gtk! {
							<Button::new_from_icon_name(Some("list-add"), IconSize::Button)
								GtkBox::expand=true valign=Align::Center
								label="Start new chat"
								on clicked=|_| UiMessage::DefineChat
							/>
						} } else { gtk!{
							<Notebook GtkBox::expand=true scrollable=true>
								<Button::new_from_icon_name(Some("list-add"), IconSize::Menu)
									Notebook::action_widget_end=true
									relief=ReliefStyle::None
									on clicked=|_| UiMessage::DefineChat
								/>
								{
									self.state.read().unwrap().chats.iter().map(move |(_, c)| gtk! {
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
