/**
  attachments are owned by the message that contains them. attachments on disk
*/
#[derive(Clone, Debug)]
pub struct Attachment {
	pub name: std::ffi::OsString,
	pub mime_type: String,
	pub data: (std::path::PathBuf, u64, u64),
}

impl Attachment {
	pub fn with_data<T, F: FnOnce(&[u8]) -> T>(&self, f: F) -> Result<T, std::io::Error> {
		let (ref path, start, len) = self.data;
		use memmap::MmapOptions;
		use std::fs::OpenOptions;
		let file = OpenOptions::new()
			.read(true)
			.write(true).open(path)?;
		let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
		let mmap = mmap.make_read_only()?;
		Ok(f(&mmap[start as usize..(start+len) as usize]))
	}
}

pub type Country = phonenumber::country::Id;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(C)]
pub struct Number {
	pub num: u64,
}

impl Number {
	pub fn new(n: u64) -> Self {
		Number { num: n }
	}

	pub fn to_string(&self) -> String {
		self.num.to_string()
	}

	fn from_phonenumber(n: phonenumber::PhoneNumber) -> Number {
		let mut formatted = format!("{}",
			n.format().mode(phonenumber::Mode::International));
		formatted = formatted.replace(" ", "");
		formatted = formatted.replace("-", "");
		let int: u64 = formatted.parse().unwrap();
		Number { num: int }
	}

	pub fn normalize(num_str: &str, default_country: Country) -> Option<Number> {
		let n = phonenumber::parse(Some(default_country), num_str).ok()?;

		if n.is_valid() {
			return Some(Self::from_phonenumber(n))
		}
		/* handle e.g. sms short codes */
		if let Ok(n) = num_str.parse() {
			Some(Number::new(n))
		} else {
			None
		}
	}

	pub fn get_country(num_str: &str) -> Option<Country> {
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
}

#[test]
fn test_normalize() {
	let base = "13104356570";
	let s1 = "3104356570";
	let s2 = "+13104356570";
	let num = Number::new(13104356570);
	let country = Number::get_country(base).unwrap();
	assert!(Number::normalize(s1, country) == Some(num));
	assert!(Number::normalize(s2, country) == Some(num));

	let num = Number::new(41411);
	let s1 = "41411";
	assert!(Number::normalize(s1, country) == Some(num));
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

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Chat {
	pub numbers: Vec<Number>,
}

impl std::borrow::Borrow<Vec<Number>> for Chat {
	fn borrow(&self) -> &Vec<Number> {
		&self.numbers
	}
}

impl Chat {
	pub fn get_name(&self, my_number: &Number) -> String {
		self.numbers.iter()
			.filter(|x| x != &my_number)
			.map(|x| x.to_string())
			.collect::<Vec<_>>().join(", ")
	}
}

use std::collections::{BTreeMap, HashMap};

pub struct VgmmsState {
	pub open_chats: Vec<Chat>,
	pub chats: BTreeMap<Chat, Option<(u64, MessageId)>>,
	pub messages: BTreeMap<MessageId, MessageInfo>,
	pub contacts: HashMap<Number, Contact>,
	pub attachments: HashMap<AttachmentId, Attachment>,
	pub next_message_id: MessageId,
	pub next_attachment_id: AttachmentId,
	pub my_number: Number,
	pub my_country: Country,
	pub modem_path: dbus::strings::Path<'static>,
	pub db_conn: rusqlite::Connection,
}
