use dbus::blocking::Connection;
use dbus::message::MatchRule;
use std::path::PathBuf;
use std::time::Duration;

use crate::types::MessageStatus;

#[derive(Debug, Clone)]
pub struct Attachment {
	pub name: String,
	pub mime_type: String,
	pub disk_path: PathBuf,
	pub start: u64,
	pub len: u64,
}

#[derive(Debug, Clone)]
pub enum DbusNotification {
	MmsStatusUpdate {
		id: [u8; 20],
		status: MessageStatus,
	},
	MmsReceived {
		id: [u8; 20],
		date: String,
		subject: Option<String>,
		sender: String,
		recipients: Vec<String>,
		attachments: Vec<Attachment>,
		smil: Option<String>,
	},
	SmsReceived {
		message: String,
		date: String,
		sender: String,
	}
}

#[derive(Debug)]
pub enum ParseError {
	BadMmsPath,
	BadArgs,
	BadSenderOrSentTime,
	BadAttachments,
	BadRecipients,
}

use DbusNotification::*;

fn parse_sms_message(msg: &dbus::Message) -> Result<DbusNotification, ParseError> {
	use dbus::arg::*;
	if let (Some(text), Some(dict)) = msg.get2::<String, Dict<&str, Variant<String>, _>>() {
		let (mut sender, mut date) = (None, None);
		for (k, v) in dict {
			match k {
				"Sender" => sender = Some(v.0),
				"SentTime" => date = Some(v.0),
				_ => (),
			}
		}
		if let (Some(sender), Some(date)) = (sender, date) {
			//println!("{} @ {}: {}", sender, date, text)
			Ok(SmsReceived {
				message: text,
				date: date,
				sender: sender,
			})
		} else {
			Err(ParseError::BadSenderOrSentTime)
		}
	} else {
		Err(ParseError::BadArgs)
	}
}

fn parse_attachments<'a>(v: &'a(dyn dbus::arg::RefArg + 'static)) -> Result<Vec<Attachment>, ParseError> {
	match || -> Option<Vec<Attachment>> {
		let mut atts = vec![];
		//descend into variant
		let mut v = v.as_iter()?;
		//descend into array of attachments
		let v = v.next()?.as_iter()?;
		for att in v {
			//iterate (ssstt) tuple
			let att_fields = att.box_clone();
			let mut att_fields = att_fields.as_iter()?;

			let name = att_fields.next()?.as_str()?.to_owned();
			let mime_type = att_fields.next()?.as_str()?.to_owned();
			let disk_path = att_fields.next()?.as_str()?.to_owned().into();
			let start = att_fields.next()?.as_u64()?;
			let len = att_fields.next()?.as_u64()?;
			atts.push(Attachment {
				name, mime_type, disk_path, start, len,
			})
		}
		Some(atts)
	}() {
		None => Err(ParseError::BadAttachments),
		Some(x) => Ok(x)
	}
}

fn parse_numbers<'a>(v: &'a(dyn dbus::arg::RefArg + 'static)) -> Result<Vec<String>, ParseError> {
	match || -> Option<Vec<String>> {
		let mut nums = vec![];
		//descend into variant
		let mut v = v.as_iter()?;
		//descend into array of numbers
		let v = v.next()?.as_iter()?;
		for num in v {
			nums.push(num.as_str()?.to_owned())
		}
		Some(nums)
	}() {
		None => Err(ParseError::BadRecipients),
		Some(x) => Ok(x)
	}
}

fn parse_mms_message(msg: &dbus::Message) -> Result<DbusNotification, ParseError> {
	use dbus::arg::*;
	if let (Some(path), Some(dict)) = msg.get2::<dbus::Path, Dict<&str, Variant<Box<dyn RefArg>>, _>>() {
		if path.len() < 40 {
			return Err(ParseError::BadMmsPath);
		}
		let mut mms_id = [0u8; 20];
		if let Err(_) = hex::decode_to_slice(&path[path.len()-40..], &mut mms_id) {
			return Err(ParseError::BadMmsPath);
		}
		let (mut sender, mut date, mut subject, mut recipients, mut attachments, mut smil) =
			(None, None, None, None, None, None);
		for (k, v) in dict {
			//println!("{}: {:?}", k, v);
			match k {
				//"Status" => status = Some(v.0),
				"Sender" => sender = v.as_str().map(|x| x.to_owned()),
				"Date" => date = v.as_str().map(|x| x.to_owned()),
				"Subject" => subject = v.as_str().map(|x| x.to_owned()),
				"Recipients" => recipients = Some(parse_numbers(&v)?),
				"Attachments" => attachments = Some(parse_attachments(&v)?),
				"Smil" => smil = v.as_str().map(|x| x.to_owned()),
				_ => (),
			}
		}
		if let (Some(sender), Some(date), Some(recipients), Some(attachments)) =
			(sender, date, recipients, attachments) {
			Ok(MmsReceived {
				id: mms_id,
				date: date,
				subject: subject,
				sender: sender,
				recipients: recipients,
				attachments: attachments,
				smil: smil,
			})
		} else {
			Err(ParseError::BadSenderOrSentTime)
		}
	} else {
		Err(ParseError::BadArgs)
	}
}

struct Conns {
	sys_conn: Connection,
	sess_conn: Connection,
}

use std::collections::HashMap;

use crate::types::{MessageItem, MessageInfo};

pub fn get_my_number(/*sys_conn: &mut Connection, */
	modem_path: &dbus::strings::Path) -> Result<Option<String>, dbus::Error> {
	let mut conn = SYS_CONN.lock().unwrap();
	let conn = conn.get_mut();
	let sim_proxy = conn.with_proxy("org.ofono", modem_path, Duration::from_millis(500));
	use crate::ofono_simmanager::OrgOfonoSimManager;
	let dict = sim_proxy.get_properties()?;
	let mut nums = None;
	for (k, v) in dict {
		if let "SubscriberNumbers" = &*k {
			 if let Ok(ns) = parse_numbers(&v) {
				nums = Some(ns)
			}
		}
	}

	let nums = match nums {
		Some(ns) => ns,
		None => return Ok(None),
	};

	Ok(if let [num] = &*nums {
		Some(num.to_owned())
	} else {
		eprintln!("expected 1 subscriber number, found {}", nums.len());
		None
	})
}

pub fn get_modem_paths(/*sys_conn: &mut Connection*/) -> Result<Vec<dbus::strings::Path<'static>>, dbus::Error> {
	let mut conn = SYS_CONN.lock().unwrap();
	let conn = conn.get_mut();
	let man_proxy = conn.with_proxy("org.ofono", "/", Duration::from_millis(500));
	use crate::ofono_manager::OrgOfonoManager;
	let modems = man_proxy.get_modems()?;
	let paths = modems.iter().map(|m| m.0.to_owned()).collect();
	Ok(paths)
}

pub fn send_message(/*sys_conn: &mut Connection, sess_conn: &mut Connection,*/
	modem_path: &dbus::strings::Path,
	msg: &MessageInfo,
	atts: &HashMap<crate::types::AttachmentId, crate::types::Attachment>) -> Result<Option<dbus::strings::Path<'static>>, dbus::Error> {

	/* prepare recipients */
	let recip_strings: Vec<_> = msg.chat.iter().filter_map(|n|
		if n != &msg.sender {
			Some(n.to_string())
		} else {
			None
		}).collect();

	/* choose sms or mms */
	if let ([recip], [MessageItem::Text(t)]) = (&*recip_strings, &*msg.contents) { /* sms */
		let mut conn = SYS_CONN.lock().unwrap();
		let conn = conn.get_mut();
		let sms_proxy = conn.with_proxy("org.ofono", modem_path, Duration::from_millis(500));
		let () = sms_proxy.method_call("org.ofono.MessageManager", "SendMessage", (recip, t))?;
		Ok(None)
	} else { /* mms */
		let recip_strs: Vec<_> = recip_strings.iter().map(|s| &s[..]).collect();
	
		/* prepare attachments */
		let mut attachments = Vec::<(&str, &str, &str)>::new(); /* name, mime type, disk path */

		let mut text_files = vec![];
		for item in &msg.contents {
			match item {
				MessageItem::Text(t) => {
					use rand::Rng;
					use std::io::Write;
					let filename = format!("{}.txt", text_files.len());
					let path = format!("/tmp/vgmms/{:x}/{}", rand::thread_rng().gen::<u32>(), filename);
					let mut f = std::fs::File::create(&filename).unwrap();
					f.write_all(t.as_bytes()).unwrap();
					text_files.push((filename, path));
				},
				_ => (),
			}
		}

		let mut n_text_files_used = 0;

		for item in &msg.contents {
			attachments.push(
				match item {
					MessageItem::Attachment(ref att_id) => {
						if let Some(att) = atts.get(att_id) {
							if att.data.1 != 0 {
								eprintln!("cannot send partial attachment!");
								continue
							}
							(att.name.to_str().unwrap(), &att.mime_type, att.data.0.to_str().unwrap())
						} else {
							eprintln!("could not find attachment {} when sending MMS", att_id);
							continue
						}
					},
					MessageItem::Text(_) => {
						let (ref filename, ref path) = &text_files[n_text_files_used];
						n_text_files_used += 1;
						(filename, "text/plain", path)
					},
				}
			);
		}

		let smil = crate::smil::generate_smil(&attachments);

		let mut conn = SESS_CONN.lock().unwrap();
		let conn = conn.get_mut();

		let mms_proxy = conn.with_proxy("org.ofono.mms", "/org/ofono/mms", Duration::from_millis(500));
		use crate::mmsd_manager::OrgOfonoMmsManager;
		let services = mms_proxy.get_services()?;
		let path: &dbus::strings::Path = &services[0].0;

		let service_proxy = conn.with_proxy("org.ofono.mms", path, Duration::from_millis(500));

		use crate::mmsd_service::OrgOfonoMmsService;
		let path = service_proxy.send_message(recip_strs, &smil, attachments)?;
		Ok(Some(path.to_owned()))
	}
}

/*pub fn start_send() -> impl futures::Sink<(MessageInfo,
	HashMap<crate::types::AttachmentId, crate::types::Attachment>)> {
	use futures::stream::StreamExt;
	let (sink, stream) = futures::channel::mpsc::channel(0);

	// open buses
	let mut sys_conn = Connection::new_system().expect("DBus connection failed");
	let mut sess_conn = Connection::new_session().expect("DBus connection failed");

	std::thread::spawn(
		move || futures::executor::block_on(
			stream.for_each(move |(m, atts)| {
				println!("sending message!");
				send_message(&mut sys_conn, &mut sess_conn, &m, &atts);
				futures::future::ready(())
		})
	));
	sink
}*/

use std::sync::{Arc, Mutex};
use std::cell::RefCell;

lazy_static! {
	pub static ref SYS_CONN: Arc<Mutex<RefCell<Connection>>> = Arc::new(Mutex::new(RefCell::new(Connection::new_system().expect("DBus connection failed"))));
	pub static ref SESS_CONN: Arc<Mutex<RefCell<Connection>>> = Arc::new(Mutex::new(RefCell::new(Connection::new_session().expect("DBus connection failed"))));
}

pub fn start_recv() -> impl futures::Stream<Item=DbusNotification> {
	let (mut sink, stream) = futures::channel::mpsc::channel(0);

	// open buses
	let mut sys_conn = Connection::new_system().expect("DBus connection failed");
	let mut sess_conn = Connection::new_session().expect("DBus connection failed");

	let sms_recv_rule = MatchRule::new_signal("org.ofono.MessageManager", "IncomingMessage");
	let mut mms_recv_rule = MatchRule::new_signal("org.ofono.mms.Service", "MessageAdded");
	mms_recv_rule.eavesdrop = true;

	let mut sms_sink = sink.clone();

	sys_conn.add_match(sms_recv_rule, move |_: (), _, msg| {
		match parse_sms_message(&msg) {
			Ok(notif) => sms_sink.try_send(notif).unwrap(),
			Err(e) => eprintln!("{:?}", e),
		};
		true
	}).expect("add_match failed");

	sess_conn.add_match(mms_recv_rule, move |_: (), _, msg| {
		match parse_mms_message(&msg) {
			Ok(notif) => sink.try_send(notif).unwrap(),
			Err(e) => eprintln!("{:?}", e),
		};
		true
	}).expect("add_match failed");

	// loop and print messages as they come
	std::thread::spawn(move || loop { sys_conn.process(Duration::from_millis(1000)).unwrap(); });
	std::thread::spawn(move || loop { sess_conn.process(Duration::from_millis(1000)).unwrap(); });
	stream
}
