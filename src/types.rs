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

	pub fn from_str(s: &str, _settings: ()) -> Option<Number> {
		Some(Number { num: s.parse().ok()? })
	}
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
