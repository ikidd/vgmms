use dbus::blocking::Connection;
use dbus::message::MatchRule;
use std::time::Duration;

use std::path::PathBuf;

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
		status: ()/*MmsStatus*/,
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

//v.as_any().downcast_ref::<Vec<String>>().map(|x| x.clone())
fn parse_recipients<'a>(v: &'a(dyn dbus::arg::RefArg + 'static)) -> Result<Vec<String>, ParseError> {
	match || -> Option<Vec<String>> {
		let mut recs = vec![];
		//descend into variant
		let mut v = v.as_iter()?;
		//descend into array of attachments
		let v = v.next()?.as_iter()?;
		for rec in v {
			recs.push(rec.as_str()?.to_owned())
		}
		Some(recs)
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
				"Recipients" => recipients = Some(parse_recipients(&v)?),
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

pub fn start() -> impl futures::Stream<Item=DbusNotification> {
	let (mut sink, stream) = futures::channel::mpsc::channel(0);

	// open buses
	let mut sess_conn = Connection::new_session().expect("DBus connection failed");
	let mut sys_conn = Connection::new_system().expect("DBus connection failed");

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
	std::thread::spawn(move || loop { sess_conn.process(Duration::from_millis(1000)).unwrap(); });
	std::thread::spawn(move || loop { sys_conn.process(Duration::from_millis(1000)).unwrap(); });
	stream
}
