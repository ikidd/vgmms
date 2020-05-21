/**
  attachments are owned by the message that contains them. attachments on disk
*/
#[derive(Clone, Debug)]
pub struct Attachment {
	pub name: std::ffi::OsString,
	pub mime_type: String,
	pub data: (std::path::PathBuf, u64, u64),
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(C)]
pub struct Number {
	pub num: u64,
}

impl Number {
	pub fn new(n: u64) -> Self {
		Number { num: n }
	}
	pub fn same(&self, other: &str) -> bool {
		other == self.to_string()
	}

	pub fn to_string(&self) -> String {
		self.num.to_string()
	}

	pub fn normalize(num_str: &str, default_country: phonenumber::country::Id) -> Option<Number> {
		let n = phonenumber::parse(Some(default_country), num_str).ok()?;
		let mut formatted = format!("{}",
			n.format().mode(phonenumber::Mode::International));
		formatted = formatted.replace(" ", "");
		formatted = formatted.replace("-", "");
		let int: u64 = formatted.parse().unwrap();
		Some(Number { num: int })
	}

	pub fn get_country(num_str: &str) -> Option<phonenumber::country::Id> {
		if let Some(n) = phonenumber::parse(None, num_str).ok() {
			if n.is_valid() {
				return n.country().id()
			}
		}

		let mut s = "+".to_owned();
		s.push_str(num_str);
		let n = phonenumber::parse(None, s).ok()?;

		if n.is_valid() {
			n.country().id()
		} else {
			None
		}
	}

	pub fn from_str(s: &str, _settings: ()) -> Option<Number> {
		Some(Number { num: s.parse().ok()? })
	}
}

#[test]
fn test_normalize() {
	let base = "13104356570";
	let s1 = "3104356570";
	let s2 = "+13104356570";
	let num = Number { num: 13104356570 };
	let country = Number::get_country(base).unwrap();
	assert!(Number::normalize(s1, country) == Some(num));
	assert!(Number::normalize(s2, country) == Some(num));
}

pub type AttachmentId = u64;
pub type MessageId = [u8; 20];

pub trait MessageIdExt {
	fn increment(&mut self);
}

impl MessageIdExt for MessageId {
	fn increment(&mut self) {
		/* bytewise increment */
		for byte in self.iter_mut().rev() {
			*byte += 1u8;
			if *byte == 0 {
				continue
			}
			break
		}
	}
}

#[derive(Clone, Debug)]
pub enum MessageItem {
	Text(String),
	Attachment(AttachmentId),
}

#[derive(Clone, Debug)]
pub enum DraftItem {
	Text(String),
	Attachment(Attachment),
}

#[derive(Clone, Debug)]
pub struct Contact {
	pub number: Number,
	pub name: String,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum MessageStatus {
	Received = 0,
	Draft,
	Sending,
	Sent,
	Failed,
}

impl MessageStatus {
	pub fn from_u8(n: u8) -> Option<MessageStatus> {
		use MessageStatus::*;
		[Received, Draft, Sending, Sent, Failed].get(n as usize).cloned()
	}
}

#[derive(Clone, Debug)]
pub struct MessageInfo {
	pub sender: Number,
	pub chat: Vec<Number>,
	pub time: u64,
	pub contents: Vec<MessageItem>,
	pub status: MessageStatus,
}

#[derive(Clone, Debug, Default)]
pub struct Chat {
	pub numbers: Vec<Number>,
}

impl Chat {
	pub fn get_name(&self, my_number: &Number) -> String {
		self.numbers.iter()
			.filter(|x| x != &my_number)
			.map(|x| x.to_string())
			.collect::<Vec<_>>().join(" ")
	}
}

use std::collections::{BTreeMap, HashMap};

pub struct VgmmsState {
	pub chats: HashMap<Vec<Number>, Chat>,
	pub messages: BTreeMap<MessageId, MessageInfo>,
	pub contacts: HashMap<Number, Contact>,
	pub attachments: HashMap<AttachmentId, Attachment>,
	pub next_message_id: MessageId,
	pub next_attachment_id: AttachmentId,
	pub my_number: Number,
	pub db_conn: rusqlite::Connection,
}
