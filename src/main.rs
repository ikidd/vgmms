#![recursion_limit="512"]

use vgtk::ext::*;
use vgtk::lib::gio::{self, ApplicationFlags};
use vgtk::lib::gtk::{*, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode};

use std::default::Default;
use std::sync::{Arc, RwLock};

use std::ffi::OsString;

mod chat;
mod dbus;
mod input_box;
mod types;
mod db;

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

		/* bytewise increment */
		for byte in self.next_message_id.iter_mut().rev() {
			*byte += 1u8;
			if *byte == 0 {
				continue
			}
			break
		}

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
					self.attachments.insert(id, att);
					contents.push(MessageItem::Attachment(id));
				}
				contents.insert(0, MessageItem::Text(text));
				
				if let Some(sender) = Number::from_str(&*sender, ()) {
					let mut chat: Vec<_> = recipients.iter().filter_map(|r| Number::from_str(&*r, ())).collect();
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
				if let Some(sender) = Number::from_str(&*sender, ()) {
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
		let my_number = Number::new(4561237890);

		let mut map = HashMap::new();
		let nums = vec![Number::new(41411), my_number];
		map.insert(nums.clone(), Chat {numbers: nums.clone()});
		let nums = vec![Number::new(1238675309), my_number];
		map.insert(nums.clone(), Chat {numbers: nums.clone()});

		let mut messages = BTreeMap::new();

		let mut conn = db::connect().unwrap();
		let _ = db::create_tables(&mut conn);

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

		VgmmsState {
			chats: map,
			messages,
			contacts: Default::default(),
			attachments: Default::default(),
			next_message_id: {let mut id = [0u8; 20]; id[19] = 1; id},
			next_attachment_id: 1,
			my_number,
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
		}
	}

	fn view(&self) -> VNode<Model> {
		gtk! {
			<Application::new_unwrap(Some("org.vgmms"), ApplicationFlags::empty())>
				<Window default_width=180 default_height=300 border_width=5 on destroy=|_| UiMessage::Exit>
					<GtkBox::new(Orientation::Vertical, 0)>
						<Notebook GtkBox::expand=true>
							{
								self.state.read().unwrap().chats.iter().map(|(_, c)| gtk! {<@ChatModel chat=c state=self.state.clone() />})
							}
						</Notebook>
					</GtkBox>
				</Window>
			</Application>
		}
	}
}

fn main() {
	use gio::prelude::ApplicationExtManual;
	use futures::stream::StreamExt;

	let notif_stream = dbus::start();
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
