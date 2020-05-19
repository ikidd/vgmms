use rusqlite::{params, Connection};
use byteorder::ByteOrder;

use crate::types::*;

pub fn connect() -> rusqlite::Result<Connection> {
	let conn = Connection::open("/tmp/test.sqlite3")?;
	Ok(conn)
}

pub fn create_tables(conn: &mut Connection) -> rusqlite::Result<usize> {
	conn.execute(
		"CREATE TABLE chats (
			numbers BLOB
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
		"INSERT INTO messages (id, sender, chat, time, contents, status)
		VALUES (X'0000001234567890e1000a0400d0000050000003', 41411, X'c3a100000000000082f7de0f01000000', 1589921285, X'7468656c6c6f00', 0)
		;", params![])?;
	conn.execute(
		"INSERT INTO messages (id, sender, chat, time, contents, status)
		VALUES (X'0000000000567833333055500000000000000008', 41411, X'c3a100000000000082f7de0f01000000', 1589921299, X'7468656c6c00', 0)
		;", params![])
}

/*
		CREATE TABLE attachments (
			id INTEGER PRIMARY KEY,
			name BLOB,
			mime_type STRING,
			start INTEGER,
			len INTEGER
		)

		CREATE TABLE chats (
			numbers BLOB
		)

*/

pub fn insert_message(conn: &mut Connection, id: &MessageId, msg: &MessageInfo) -> rusqlite::Result<usize> {
	let chat_bytes: &[u8] = unsafe {
		std::slice::from_raw_parts(
			msg.chat.as_ptr() as *const _,
			msg.chat.len() * std::mem::size_of::<Number>())
	};
	
	println!("insert chats: {:?}", chat_bytes);
	
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
				byteorder::LittleEndian::write_u64(&mut contents_bytes, *att_id);
				contents_bytes.push(0);
			}
		}
	}

	println!("insert contents: {:?}", contents_bytes);
	
	conn.execute(
		"INSERT INTO messages (id, sender, chat, time, contents, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
		params![&id[..], msg.sender.num as i64, chat_bytes, msg.time as i64, contents_bytes, msg.status as u8],
	)
}

pub struct Query<'a>(rusqlite::Statement<'a>);

impl<'a> Query<'a> {
	pub fn new(conn: &'a mut Connection) -> rusqlite::Result<Query<'a>> {
		Ok(Query(conn.prepare("SELECT id, sender, chat, time, contents, status FROM messages ORDER BY time")?))
	}
}

pub fn get_all_messages<'a>(stmt: &'a mut Query) -> rusqlite::Result<Result<impl Iterator<Item=rusqlite::Result<(MessageId, MessageInfo)>> + 'a, String>> {
	use std::convert::TryInto;

	fn get_u8(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<u8> {
		Ok(row.get::<_, i8>(idx)? as u8)
	}

	fn get_u64(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<u64> {
		Ok(row.get::<_, i64>(idx)? as u64)
	}

	fn get_number(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Number> {
		Ok(Number { num: get_u64(row, idx)? })
	}

	fn get_numbers(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Vec<Number>> {
		if let rusqlite::types::ValueRef::Blob(data) = row.get_raw(idx) {
			let chat_nums: &[Number] = unsafe {
				std::slice::from_raw_parts(
					data.as_ptr() as *const _,
					data.len() / std::mem::size_of::<Number>())
			};
			Ok(chat_nums.to_vec())
		} else {
			/* value was not a blob! */
			Err(rusqlite::Error::InvalidColumnType(idx, "Vec<Number>".into(), rusqlite::types::Type::Blob))
		}
	}

	//TODO: this is an awful serialization scheme but works for message text not containing NUL bytes
	fn get_message_items(row: &rusqlite::Row, idx: usize) -> rusqlite::Result<Vec<MessageItem>> {
		/*
		serialization format: 
			't' [^\0]+ '\0'
			'a' .{8} '\0'
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

	let message_iter = stmt.0.query_map(params![], |row| {
		let id: [u8; 20] = if let rusqlite::types::ValueRef::Blob(data) = row.get_raw(0) {
			data.try_into().expect("invalid message ID")
		} else {
			panic!("id column contained non-blob type")
		};
		let message = MessageInfo {
			sender: get_number(row, 1).expect("invalid type"),
			chat: get_numbers(row, 2).expect("invalid type"),
			time: get_u64(row, 3).expect("invalid type"),
			contents: get_message_items(row, 4).expect("invalid type"),
			status: MessageStatus::from_u8(get_u8(row, 5).expect("invalid type")).expect("invalid message status"),
		};
		Ok((id, message))
	})?;

	Ok(Ok(message_iter))
}
