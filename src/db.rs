use rusqlite::{params, Connection, OptionalExtension};
use byteorder::WriteBytesExt;

use crate::types::*;
use self::get::*;

pub fn connect() -> rusqlite::Result<Connection> {
	let mut path = xdg_basedir::get_data_home()
		.expect("could not find XDG data directory");
	path.push("vgmms");
	std::fs::create_dir_all(&path)
		.expect(&format!("could not create {}", path.display()));
	path.push("vgmms.db");
	let conn = Connection::open(path)?;
	Ok(conn)
}

pub fn create_tables(conn: &mut Connection) -> rusqlite::Result<usize> {
	conn.execute(
		"CREATE TABLE chats (
			numbers BLOB PRIMARY KEY,
			tab_id INTEGER UNIQUE,
			last_msg_id BLOB
		)", params![])?;
	conn.execute(
		"CREATE TABLE messages (
			id BLOB PRIMARY KEY,
			sender INTEGER,
			chat BLOB,
			time INTEGER,
			contents BLOB,
			status INTEGER
		)", params![])?;
	conn.execute(
		"CREATE TABLE attachments (
			id INTEGER PRIMARY KEY,
			name BLOB,
			mime_type STRING,
			path BLOB,
			start INTEGER,
			len INTEGER
		)", params![])
/* red.png:
89504e470d0a1a0a0000000d4948445200000064
0000006401030000004a2c071700000003504c54
4592000059ed5144000000134944415418196318
05a360148c82514057000005780001dc45021b00
00000049454e44ae426082
*/
/*	conn.execute(
		"INSERT INTO attachments (id, name, mime_type, path, start, len)
		VALUES (4, X'7265642e706e67', 'image/png', X'2f746d702f7265642e706e67', 0, 91)
		;", params![])?;
	conn.execute(
		"INSERT INTO messages (id, sender, chat, time, contents, status)
		VALUES (X'0000001234567890e1000a0400d0000050000003', 41411, X'c3a100000000000082f7de0f01000000', 1589921285, X'7468656c6c6f00', 0)
		;", params![])?;
	conn.execute(
		"INSERT INTO messages (id, sender, chat, time, contents, status)
		VALUES (X'0000000000567833333055500000000000000008', 41411, X'c3a100000000000082f7de0f01000000', 1589921299, X'610400000000000000', 0)
		;", params![])*/
}

fn chat_to_bytes(chat: &[Number]) -> &[u8] {
	unsafe {
		std::slice::from_raw_parts(
			chat.as_ptr() as *const _,
			chat.len() * std::mem::size_of::<Number>())
	}
}

unsafe fn bytes_to_chat(data: &[u8]) -> &[Number] {
	std::slice::from_raw_parts(
		data.as_ptr() as *const _,
		data.len() / std::mem::size_of::<Number>())
}

pub fn open_chat(conn: &mut Connection, chat: &Chat, tab_id: i32) -> rusqlite::Result<()> {
	assert!(tab_id >= 0);

	let chat_bytes = chat_to_bytes(&*chat.numbers);
	let tx = conn.transaction()?;
	/* increase tab numbers after this one */
	tx.execute("UPDATE chats SET tab_id = tab_id + 1 WHERE tab_id >= ?1;",
		params![tab_id],
	)?;

	/* get old last_msg_id */
	let mut q = tx.prepare("SELECT last_msg_id FROM chats \
		WHERE numbers = ?1")?;

	let last_msg_id = q.query_row(params![chat_bytes], |row| {
		Ok(get_id(row, 0).unwrap_or([0u8; 20]))
	})
		.optional()?
		.unwrap_or([0u8; 20]);

	drop(q);

	/* update this chat */
	tx.execute(
		"INSERT INTO chats (numbers, tab_id, last_msg_id) VALUES (?1, ?2, ?3);",
		params![chat_bytes, tab_id, &last_msg_id[..]],
	)?;
	tx.commit()
}

pub fn set_chat_tab(conn: &mut Connection, chat: &Chat, tab_id: i32) -> rusqlite::Result<usize> {
	conn.execute(
		"UPDATE chats SET tab_id = ?1 WHERE numbers = ?2;",
		params![tab_id, chat_to_bytes(&*chat.numbers)],
	)
}

pub fn close_chat(conn: &mut Connection, chat: &Chat) -> rusqlite::Result<usize> {
	conn.execute(
		"UPDATE chats SET tab_id = NULL WHERE numbers = ?1;",
		params![chat_to_bytes(&*chat.numbers)],
	)
}

pub fn insert_message(conn: &mut Connection, id: &MessageId, msg: &MessageInfo) -> rusqlite::Result<()> {
	let chat_bytes: &[u8] = chat_to_bytes(&*msg.chat);

	let mut contents_bytes = vec![];
	for m in &msg.contents {
		use std::io::Write;
		match m {
			MessageItem::Text(t) => {
				contents_bytes.push(b't');
				let _ = contents_bytes.write_all(t.as_bytes());
				contents_bytes.push(0);
			}
			MessageItem::Attachment(att_id) => {
				contents_bytes.push(b'a');
				let _ = contents_bytes.write_u64::<byteorder::LittleEndian>(*att_id);
			}
		}
	}

	let tx = conn.transaction()?;
	tx.execute(
		"INSERT INTO messages (id, sender, chat, time, contents, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
		params![&id[..], msg.sender.num as i64, chat_bytes, msg.time as i64, contents_bytes, msg.status as u8],
	)?;
	tx.execute(
		"UPDATE chats SET last_msg_id = ?1 where numbers = ?2;",
		params![&id[..], chat_bytes],
	)?;
	tx.commit()
}

pub fn insert_attachment(conn: &mut Connection, id: &AttachmentId, att: &Attachment) -> rusqlite::Result<usize> {
	use std::os::unix::ffi::OsStrExt;
	conn.execute(
		"INSERT INTO attachments (id, name, mime_type, path, start, len) VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
		params![*id as i64, att.name.as_bytes(), att.mime_type,
			att.data.0.as_os_str().as_bytes(), att.data.1 as i64, att.data.2 as i64],
	)
}

pub struct Query<'a>(rusqlite::Statement<'a>);

impl<'a> Query<'a> {
	pub fn new(conn: &'a mut Connection) -> rusqlite::Result<Query<'a>> {
		Ok(Query(conn.prepare("SELECT id, sender, chat, time, contents, status FROM messages ORDER BY time")?))
	}
}

pub fn get_next_message_id(conn: &mut Connection) -> rusqlite::Result<MessageId> {
	let mut stmt = conn.prepare("SELECT max(id) FROM messages")?;
	stmt.query_row(params![], |row| {
		if let Ok(mut id) = get_id(row, 0) {
			id.increment();
			Ok(id)
		} else {
			Err(rusqlite::Error::InvalidColumnType(0, "message id".into(), rusqlite::types::Type::Blob))
		}
	})
}

pub fn get_next_attachment_id(conn: &mut Connection) -> rusqlite::Result<AttachmentId> {
	let mut stmt = conn.prepare("SELECT max(id) FROM attachments")?;
	let mut iter = stmt.query_map(params![], |row| {
		if let Ok(id) = get_u64(row, 0) {
			Ok(id + 1)
		} else {
			Err(rusqlite::Error::InvalidColumnType(0, "attachment id".into(), rusqlite::types::Type::Blob))
		}
	})?;
	iter.next().unwrap()
}

/*
	return chats along with their open tab index (-1 if closed) and last message (if any) timestamp + id
*/
pub fn get_all_chats(conn: &mut Connection) -> rusqlite::Result<Vec<(Chat, i32, Option<(u64, MessageId)>)>> {
	let mut q = conn.prepare("SELECT numbers, tab_id, last_msg_id, time FROM chats \
		LEFT JOIN messages ON chats.last_msg_id = messages.id \
		ORDER BY tab_id")?;

	let chat_iter = q.query_map(params![], |row| {
		let chat = Chat {
			numbers: get_numbers(row, 0)?,
		};
		let tab_id: i32 = row.get(1).unwrap_or(-1);
		let last_msg_info = match (get_id(row, 2), get_u64(row, 3)) {
			(Ok(msg_id), Ok(timestamp)) => Some((timestamp, msg_id)),
			_ => None,
		};
		Ok((chat, tab_id, last_msg_info))
	})?;

	Ok(chat_iter
		.inspect(|x| if let Err(e) = x {
			eprintln!("error loading chat: {}", e)
		})
		.filter_map(Result::ok).collect())
}

pub fn get_all_messages<'a>(stmt: &'a mut Query) -> rusqlite::Result<Result<impl Iterator<Item=rusqlite::Result<(MessageId, MessageInfo)>> + 'a, String>> {
	let message_iter = stmt.0.query_map(params![], |row| {
		let id: MessageId = get_id(row, 0)?;
		let message = MessageInfo {
			sender: get_number(row, 1)?,
			chat: get_numbers(row, 2)?,
			time: get_u64(row, 3)?,
			contents: get_message_items(row, 4)?,
			status: MessageStatus::from_u8(get_u8(row, 5)?).expect("invalid message status"),
		};
		Ok((id, message))
	})?;

	Ok(Ok(message_iter))
}

use std::collections::HashMap;

pub fn get_all_attachments(conn: &mut Connection) -> rusqlite::Result<HashMap<AttachmentId, Attachment>> {
	let mut q = conn.prepare("SELECT id, name, mime_type, path, start, len FROM attachments")?;

	let att_iter = q.query_map(params![], |row| {
		let id: AttachmentId = get_u64(row, 0)?;
		use std::os::unix::ffi::OsStringExt;
		let path: std::ffi::OsString = OsStringExt::from_vec(row.get(3)?);
		let att = Attachment {
			name: OsStringExt::from_vec(row.get(1)?),
			mime_type: row.get::<_, String>(2)?,
			data: (
				path.into(),
				get_u64(row, 4)?,
				get_u64(row, 5)?,
			),
		};
		Ok((id, att))
	})?;

	Ok(att_iter
		.inspect(|x| if let Err(e) = x {
			eprintln!("error loading attachment: {}", e)
		})
		.filter_map(Result::ok).collect())
}

mod get {
	use byteorder::ByteOrder;

	use crate::types::*;

	pub fn get_u8(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<u8> {
		Ok(row.get::<_, i8>(idx)? as u8)
	}

	pub fn get_u64(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<u64> {
		Ok(row.get::<_, i64>(idx)? as u64)
	}

	pub fn get_number(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Number> {
		Ok(Number { num: get_u64(row, idx)? })
	}

	pub fn get_id(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<MessageId> {
		use std::convert::TryInto;
		let id: MessageId = if let rusqlite::types::ValueRef::Blob(data) = row.get_raw(idx) {
			data.try_into().expect("invalid message ID")
		} else {
			return Err(rusqlite::Error::InvalidColumnType(idx, "id".into(), rusqlite::types::Type::Blob))
		};
		Ok(id)
	}

	pub fn get_numbers(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Vec<Number>> {
		if let rusqlite::types::ValueRef::Blob(data) = row.get_raw(idx) {
			let chat_nums = unsafe { crate::db::bytes_to_chat(data) };
			Ok(chat_nums.to_vec())
		} else {
			/* value was not a blob! */
			Err(rusqlite::Error::InvalidColumnType(idx, "Vec<Number>".into(), rusqlite::types::Type::Blob))
		}
	}

	//TODO: this is an awful serialization scheme but works for message text not containing NUL bytes
	pub fn get_message_items(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Vec<MessageItem>> {
		/*
		serialization format: 
			't' [^\0]+ '\0'
			'a' .{8}
		*/

		enum Kind { Unknown, Text, Attachment, };
		use Kind::*;

		let mut contents = vec![];

		let vec = row.get::<_, Vec<u8>>(idx)?;
		let mut kind = Unknown;
		let mut text_start = 0;

		let mut i = 0;
		while i < vec.len() {
			let n = vec[i];
			match kind {
				Unknown => match n {
					b't' => { kind = Text; text_start = i + 1; },
					b'a' => { kind = Attachment; },
					_ => panic!("invalid message content in db"),
				},
				Text => if n == 0 {
					let s = match std::str::from_utf8(&vec[text_start..i]) {
						Ok(s) => s,
						Err(e) => return Err(rusqlite::Error::Utf8Error(e)),
					};
					contents.push(MessageItem::Text(s.to_owned()));
					kind = Unknown;
				},
				Attachment => {
					let len = std::mem::size_of::<u64>();
					let buf = &vec[i..i+len];
					let att_id = byteorder::LittleEndian::read_u64(buf);
					contents.push(MessageItem::Attachment(att_id));
					i += len - 1;
					kind = Unknown;
				},
			}
			i += 1;
		}
		Ok(contents)
	}
}
