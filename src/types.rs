#[derive(Clone, Debug)]
pub enum AttachmentData {
	Inline(Vec<u8>),
	FileRef(std::path::PathBuf, u64, u64),
}

/**
  attachments are owned by the message that contains them. attachments on disk
*/
#[derive(Clone, Debug)]
pub struct Attachment {
	pub name: std::ffi::OsString,
	pub mime_type: String,
	pub size: u64,
	pub data: AttachmentData,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Number {
	num: u64,
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

pub type AttachmentId = usize;
pub type MessageId = [u8; 20];

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
#[derive(Clone, Debug)]
pub enum MessageStatus {
	Received,
	Draft,
	Sending,
	Sent,
	Failed,
}

#[derive(Clone, Debug)]
pub struct MessageInfo {
	pub sender: Number,
	pub recipients: Vec<Number>,
	pub time: u64,
	pub contents: Vec<MessageItem>,
	pub status: MessageStatus,
}

#[derive(Clone, Debug, Default)]
pub struct Chat {
	pub numbers: Vec<Number>,
}
