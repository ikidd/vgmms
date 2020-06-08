use std::default::Default;

use std::ffi::OsString;
use crate::{db, dbus, types::*};

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

	pub fn add_message(&mut self, id: MessageId, message: MessageInfo) {
		if let Err(e) = db::insert_message(&mut self.db_conn, &id, &message) {
			eprintln!("error saving message: {}", e);
		}
		self.messages.insert(id, message);
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
					let mut chat: Vec<_> = recipients.iter()
						.filter_map(|r| Number::normalize(&*r, self.my_country)).collect();
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
					self.add_message(id, message);
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
					self.add_message(id, message);
				} else {
					eprintln!("cannot parse number {}", sender);
				}
			}
		}
	}
}

use std::collections::BTreeMap;

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

		let mut chats = BTreeMap::new();
		let mut open_chats = vec![];
		for (c, tab_id, last_msg_info) in db::get_all_chats(&mut conn).unwrap().into_iter() {
			/* insert into open_chats if open */
			if tab_id >= 0 {
				let tab_id = tab_id as usize;
				/* ensure sufficient room in open_chats */
				while open_chats.len() <= tab_id {
					open_chats.push(Default::default());
				}
				open_chats[tab_id] = c.clone();
			}
			/* insert into chats map */
			chats.insert(c, last_msg_info);
		}

		VgmmsState {
			open_chats,
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
