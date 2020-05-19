#![recursion_limit="512"]

use vgtk::ext::*;
use vgtk::lib::gio::{self, ApplicationFlags};
use vgtk::lib::gtk::{self, *, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode, Callback};

use vgtk::lib::gdk_pixbuf::Pixbuf;

use std::default::Default;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::boxed::Box;

use std::ffi::OsString;

mod dbus;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
struct Number {
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

#[derive(Clone, Default)]
struct Model {
	state: Arc<RwLock<VgmmsState>>,
}

#[derive(Clone, Debug)]
enum AttachmentData {
	Inline(Vec<u8>),
	FileRef(PathBuf, u64, u64),
}

/**
  attachments are owned by the message that contains them. attachments on disk
*/
#[derive(Clone, Debug)]
struct Attachment {
	name: OsString,
	mime_type: String,
	size: u64,
	data: AttachmentData,
}

type AttachmentId = usize;
type MessageId = [u8; 20];

#[derive(Clone, Debug)]
enum MessageItem {
	Text(String),
	Attachment(AttachmentId),
}

#[derive(Clone, Debug)]
enum DraftItem {
	Text(String),
	Attachment(Attachment),
}

#[derive(Clone, Debug)]
struct Contact {
	number: Number,
	name: String,
}

#[derive(Clone, Debug)]
enum MessageStatus {
	Received,
	Draft,
	Sending,
	Sent,
	Failed,
}

#[derive(Clone, Debug)]
struct MessageInfo {
	sender: Number,
	recipients: Vec<Number>,
	time: u64,
	contents: Vec<MessageItem>,
	status: MessageStatus,
}

struct MessageTarget {
}


#[derive(Clone, Debug, Default)]
struct Chat {
//	OneOnOne(Number),
	numbers: Vec<Number>,
}

impl Chat {
}

#[derive(Clone, Debug)]
enum UiMessage {
	Notif(dbus::DbusNotification),
	Send(Vec<MessageItem>, Chat),
	AskDelete(MessageId),
	Delete(MessageId),
	Exit,
}

use std::collections::{HashMap, BTreeMap};

struct VgmmsState {
	chats: HashMap<Vec<Number>, Chat>,
	messages: BTreeMap<MessageId, MessageInfo>,
	contacts: HashMap<Number, Contact>,
	attachments: HashMap<AttachmentId, Attachment>,
	next_message_id: MessageId,
	next_attachment_id: usize,
	my_number: Number,
}

fn read_file_chunk(path: &std::path::Path, start: u64, len: u64) -> Result<Vec<u8>, std::io::Error> {
	use std::io::{Read, Seek, SeekFrom};

	let mut file = std::fs::File::open(path)?;
	file.seek(SeekFrom::Start(start))?;
	let mut out = vec![0; len as usize];
	file.read_exact(&mut out[..])?;
	Ok(out)
}

impl VgmmsState {
	pub fn next_message_id(&mut self) -> MessageId {
		let id = self.next_message_id;

		/* bytewise increment */
		let mut carry = true;
		for byte in self.next_message_id.iter_mut().rev() {
			*byte += carry as u8;
			if *byte == 0 && carry {
				continue
			}
			break
		}

		id
	}

	pub fn next_attachment_id(&mut self) -> usize {
		let id = self.next_attachment_id;
		self.next_attachment_id += 1;
		id
	}

	pub fn handle_notif(&mut self, notif: dbus::DbusNotification) {
		use self::dbus::DbusNotification::*;
		match notif {
			MmsStatusUpdate {
				id: _, status: _,
			} => (),
			MmsReceived {
				id: _, date, subject: _, sender,
				recipients, attachments,
				smil: _,
			} => {
				let date = match chrono::DateTime::parse_from_rfc3339(&date) {
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
						size: att.len,
						data: AttachmentData::FileRef(att.disk_path, att.start, att.len),
					};
					self.attachments.insert(id, att);
					contents.push(MessageItem::Attachment(id));
				}
				contents.insert(0, MessageItem::Text(text));
				if let Some(num) = Number::from_str(&*sender, ()) {
					let id = self.next_message_id();
					self.messages.insert(id, MessageInfo {
						sender: num,
						recipients: recipients.iter().filter_map(|r| Number::from_str(&*r, ())).collect(),
						time: date.timestamp() as u64,
						contents: contents,
						status: MessageStatus::Received,
					});
				} else {
					eprintln!("cannot parse number {}", sender);
				}
			},
			SmsReceived {
				message, date, sender,
			} => {
				let date = match chrono::DateTime::parse_from_rfc3339(&date) {
					Ok(d) => d,
					Err(e) => {
						eprintln!("cannot parse timestamp {}: {}", date, e);
						return
					},
				};
				if let Some(num) = Number::from_str(&*sender, ()) {
					let id = self.next_message_id();
					self.messages.insert(id, MessageInfo {
						sender: num,
						recipients: vec![self.my_number],
						time: date.timestamp() as u64,
						contents: vec![MessageItem::Text(message)],
						status: MessageStatus::Received,
					});
				} else {
					eprintln!("cannot parse number {}", sender);
				}
			}
		}
	}
}

//fn ensure_chat_for(&mut self, recipients: ) -> 

impl Default for VgmmsState {
	fn default() -> Self {
		let mut map = HashMap::new();
		let nums = vec![Number::new(41411)];
		map.insert(nums.clone(), Chat {numbers: nums.clone()});
		let nums = vec![Number::new(1238675309)];
		map.insert(nums.clone(), Chat {numbers: nums.clone()});
		VgmmsState {
			chats: map,
			messages: Default::default(),
			contacts: Default::default(),
			attachments: Default::default(),
			next_message_id: {let mut id = [0u8; 20]; id[19] = 1; id},
			next_attachment_id: 1,
			my_number: Number::new(4561237890),
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
				scope.try_send(UiMessage::Notif(notif));
				futures::future::ready(())
			}))
	);
	std::process::exit(app.run(&[]));
}



#[derive(Clone, Debug, Default)]
struct FileChooser {
	on_choose: Callback<Vec<PathBuf>>
}

#[derive(Clone, Debug)]
enum UiMessageFileChooser {
	Choose(Vec<PathBuf>),
	Nop,
}

impl Component for FileChooser {
	type Message = UiMessageFileChooser;
	type Properties = Self;

	fn create(props: Self) -> Self {
		props
	}

	fn change(&mut self, props: Self) -> UpdateAction<Self> {
		*self = props;
		UpdateAction::Render
	}

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		if let UiMessageFileChooser::Choose(fns) = msg {
			self.on_choose.send(fns);
		}
		UpdateAction::None
	}

	fn view(&self) -> VNode<Self> {
		gtk! {
			<FileChooserDialog::with_buttons(Some("Select attachment"), None::<&gtk::Window>,
				FileChooserAction::Open,
				&[("_Cancel", ResponseType::Cancel), ("_Open", ResponseType::Accept)])
				on realize=|chooser| {chooser.set_select_multiple(true); UiMessageFileChooser::Nop}
				on response=|chooser, _resp| UiMessageFileChooser::Choose(chooser.get_filenames())
			/>
		}
	}
}

#[derive(Clone, Default)]
struct InputBoxModel {
	file_paths: Vec<PathBuf>,
	message: String,
	on_send: Callback<Vec<DraftItem>>,
}

#[derive(Clone, Debug)]
enum UiMessageInputBox {
	Send,
	TextChanged(String),
	ToggleFile,
	AskForFile,
	AddFile(PathBuf),
	SetFiles(Vec<PathBuf>),
	ClearFiles,
	Clear,
	Nop,
}

fn once<A, F: FnOnce(A)>(f: F) -> impl Fn(A) {
    use std::cell::Cell;
    use std::rc::Rc;

    let f = Rc::new(Cell::new(Some(f)));
    move |value| {
        if let Some(f) = f.take() {
            f(value);
        } else {
            panic!("vgtk::once() function called twice 😒");
        }
    }
}

impl Component for InputBoxModel {
	type Message = UiMessageInputBox;
	type Properties = Self;

	fn create(props: Self) -> Self {
		props
	}

	fn change(&mut self, props: Self) -> UpdateAction<Self> {
		*self = props;
		UpdateAction::Render
	}

	fn update(&mut self, mut msg: Self::Message) -> UpdateAction<Self> {
		use UiMessageInputBox::*;
		if let ToggleFile = msg {
			msg = if self.file_paths.len() == 0 { UiMessageInputBox::AskForFile } else { UiMessageInputBox::Clear };
		}
		match msg {
			Send => {
				let mut items = vec![];
				if self.message.len() > 0 {
					let mut s = String::new();
					std::mem::swap(&mut self.message, &mut s);
					items.push(DraftItem::Text(s));
				}
				for path in self.file_paths.drain(..) {
					let filename = path.file_name().unwrap_or_default().into();
					let size = match path.metadata() {
						Ok(meta) => meta.len(),
						Err(e) => {
							eprintln!("could not stat file: {}", path.display());
							continue
						},
					};
					let att = Attachment {
						name: filename,
						mime_type: tree_magic::from_filepath(&path),
						size: size,
						data: AttachmentData::FileRef(path, 0, size),
					};
					items.push(DraftItem::Attachment(att));
				}
				self.on_send.send(items);
				self.message = String::new();
				UpdateAction::Render
			},
			TextChanged(s) => {
				self.message = s;
				UpdateAction::None
			},
			AskForFile => {
				let (notify, fns_result) = futures::channel::oneshot::channel();

				let fut = vgtk::run_dialog_props::<FileChooser>(vgtk::current_window().as_ref(),
					FileChooser {
						on_choose: {let cb: Callback<Vec<PathBuf>> = Box::new(once(move |filenames| {
							let _ = notify.send(filenames);
						})).into(); cb},
					});

				let fut = async move {
					if let Ok(ResponseType::Accept) = fut.await {
						let filenames = fns_result.await.unwrap();
						SetFiles(filenames)
					} else {
						Nop
					}
				};

				UpdateAction::Defer(Box::pin(fut))
			}
			AddFile(path) => {
				self.file_paths.push(path);
				UpdateAction::Render
			},
			SetFiles(paths) => {
				self.file_paths = paths;
				UpdateAction::Render
			},
			ClearFiles => {
				self.file_paths.clear();
				UpdateAction::Render
			},
			Clear => {
				self.file_paths.clear();
				self.message.clear();
				UpdateAction::Render
			},
			_ => {
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<Self> {
		let files_empty = self.file_paths.len() == 0;
		gtk! {
			<GtkBox::new(Orientation::Horizontal, 0)>
				/*<Button label="" image="mail-attachment" always_show_image=true
					on clicked=|_entry| UiMessageInputBox::AskForFile
				/>*/
				/*<Button label="" image="edit-clear" /*visible={self.file_paths.len() > 0}*/ always_show_image=true
					on clicked=|_entry| UiMessageInputBox::ClearFiles
				/>*/
				<Entry
					text=self.message.clone()
					GtkBox::expand=true
					property_secondary_icon_name={if files_empty { "mail-attachment" } else { "edit-clear" }}
					on icon_press=|_entry, _pos, _ev| UiMessageInputBox::ToggleFile
					on realize=|entry| { entry.grab_focus(); UiMessageInputBox::Nop }
					on changed=|entry| {
						let text = entry.get_text().map(|x| x.to_string()).unwrap_or_default();
						UiMessageInputBox::TextChanged(text)
					}
					on activate=|_| UiMessageInputBox::Send
				/>
				<Button label="" image="go-next" always_show_image=true
					on clicked=|_| UiMessageInputBox::Send
				/>
			</GtkBox>
		}
	}
}


#[derive(Clone, Default)]
struct ChatModel {
	state: Arc<RwLock<VgmmsState>>,
	chat_log: Vec<MessageId>,
	chat: Chat,
}

#[derive(Clone, Debug)]
enum UiMessageChat {
	NewMessage(MessageId),
	Send(Vec<DraftItem>),
	AskDelete(MessageId),
	Delete(MessageId),
}

impl ChatModel {
	fn generate_log_widgets<'a>(&'a self, state: &'a VgmmsState) -> impl Iterator<Item=VNode<Self>> + 'a {
		state.messages.iter().filter_map(move |(_id, msg)| {
			if msg.recipients != self.chat.numbers {
				return None
			}
			let (align, halign) = match msg.status {
				MessageStatus::Received => (0.0, gtk::Align::Start),
				_ => (1.0, gtk::Align::End),
			};
			let widget_content = msg.contents.iter().map(|item| {
				match item {
					MessageItem::Text(ref t) => {
						let text = format!("[{}] {}: {}", msg.time, msg.sender.num, t);
						gtk! { <Label label=text line_wrap=true line_wrap_mode=pango::WrapMode::WordChar xalign=align /> }
					},
					MessageItem::Attachment(ref id) => {
						let att = state.attachments.get(id).expect("attachment not found!");
						if true /*mime_type_is_image(att.mime_type)*/ {
							if let AttachmentData::FileRef(ref path, start, len) = att.data {
								/*gtk! { <Image file=path /> }*/
								if let Ok(pixbuf) = Pixbuf::new_from_file_at_size(path, 200, 200) {
									gtk! { <Image pixbuf=Some(pixbuf) halign=halign /> }
								} else {
									gtk! { <Label label="unloadable image" xalign=align /> }
								}
							} else {
								gtk! { <Label label="image data not found" xalign=align /> }
							}
						} else {
							let text = format!("attachment of type {}", att.mime_type);
							gtk! { <Label label=text xalign=align /> }
						}
					},
				}
			});
			Some(gtk! {
				<ListBoxRow>
					<GtkBox::new(Orientation::Vertical, 0)>
						{widget_content}
					</GtkBox>
				</ListBoxRow>
			})
		})
	}
}

impl Component for ChatModel {
	type Message = UiMessageChat;
	type Properties = Self;

	fn create(props: Self) -> Self {
		props
	}

	fn change(&mut self, props: Self) -> UpdateAction<Self> {
		*self = props;
		UpdateAction::Render
	}

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		use UiMessageChat::*;
		match msg {
			NewMessage(id) => {
				self.chat_log.push(id);
				UpdateAction::Render
			},
			Send(draft_items) => {
				if draft_items.len() == 0 {
					return UpdateAction::None
				}
				let items = {
					let mut state = self.state.write().unwrap();
					draft_items.into_iter().map(|item| match item {
						DraftItem::Attachment(att) =>
							MessageItem::Attachment({
								let id = state.next_attachment_id();
								state.attachments.insert(id, att);
								id
							}),
						DraftItem::Text(t) => MessageItem::Text(t),
						})
					.collect()
				};
				let id = {
					let mut state = self.state.write().unwrap();
					let id = state.next_message_id();
					let num = state.my_number;
					state.messages.insert(id, MessageInfo {
						sender: num,
						recipients: self.chat.numbers.clone(),
						time: chrono::offset::Local::now().timestamp() as u64,
						contents: items,
						status: MessageStatus::Sending,
					});
					id
				};
				let fut = async move {
					NewMessage(id)
				};
				UpdateAction::Defer(Box::pin(fut))
			},
			AskDelete(_msg_id) => {
				UpdateAction::None
			},
			Delete(_msg_id) => {
				UpdateAction::Render
			},
		}
	}

	fn view(&self) -> VNode<ChatModel> {
		let state = self.state.read().unwrap();
		gtk! {
			<GtkBox::new(Orientation::Vertical, 0)>
				<ScrolledWindow GtkBox::expand=true>
					<ListBox> //TODO: TreeView
					{self.generate_log_widgets(&*state)}
					</ListBox>
				</ScrolledWindow>
				<@InputBoxModel
					on send=|draft| UiMessageChat::Send(draft)
				/>
			</GtkBox>
		}
	}
}
