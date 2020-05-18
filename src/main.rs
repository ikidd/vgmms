#![recursion_limit="512"]

use vgtk::ext::*;
use vgtk::lib::gio::{ApplicationFlags};
use vgtk::lib::gtk::*;
use vgtk::{gtk, run, Component, UpdateAction, VNode};

use vgtk::lib::gdk_pixbuf::Pixbuf;

use std::default::Default;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

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
enum UiMessageTriv {
	Exit,
}

enum AttachmentData {
	Inline(Vec<u8>),
	FilePath(PathBuf),
}

/**
  attachments are owned by the message that contains them. attachments on disk
*/
struct Attachment {
	filename: Vec<u8>,
	mime_type: String,
	size: u64,
	data: AttachmentData,
}

type AttachmentId = usize;
type MessageId = usize;

#[derive(Clone, Debug)]
enum MessageItem {
	Text(String),
	Attachment(AttachmentId),
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
	Send(Vec<MessageItem>, Chat),
	AskDelete(MessageId),
	Delete(MessageId),
	Exit,
}

fn on_dbus() {
//	let targets
}
use std::collections::HashMap;

struct VgmmsState {
	chats: HashMap<Vec<Number>, Chat>,
	messages: HashMap<MessageId, MessageInfo>,
	contacts: HashMap<Number, Contact>,
	attachments: HashMap<AttachmentId, Attachment>,
	next_message_id: usize,
	next_attachment_id: usize,
	my_number: Number,
}

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
			next_message_id: 4321,
			next_attachment_id: 1,
			my_number: Number::new(4561237890),
		}
	}
}

impl Component for Model {
	type Message = UiMessageTriv;
	type Properties = ();

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		match msg {
			UiMessageTriv::Exit => {
				vgtk::quit();
				UpdateAction::None
			}
		}
	}

	fn view(&self) -> VNode<Model> {
		gtk! {
			<Application::new_unwrap(Some("org.vgmms"), ApplicationFlags::empty())>
				<Window default_width=180 default_height=300 border_width=5 on destroy=|_| UiMessageTriv::Exit>
					<Box::new(Orientation::Vertical, 0)>
						<Notebook Box::expand=true>
							{
								self.state.read().unwrap().chats.iter().map(|(_, c)| gtk! {<@ChatModel chat=c />})
							}
						</Notebook>
					</Box>
				</Window>
			</Application>
		}
	}
}

fn main() {
	pretty_env_logger::init();
	std::process::exit(run::<Model>());
}





#[derive(Clone, Default)]
struct ChatModel {
	state: Arc<RwLock<VgmmsState>>,
	chat_log: Vec<MessageId>,
}

#[derive(Clone, Debug)]
enum UiMessageChat {
	NewMessage(MessageId),
	Send(Vec<MessageItem>),
	AskDelete(MessageId),
	Delete(MessageId),
	Nop,
}

impl ChatModel {
	fn generate_log_widgets<'a>(&'a self, state: &'a VgmmsState) -> impl Iterator<Item=VNode<Self>> + 'a {
		self.chat_log.iter().filter_map(move |id| {
			let msg = state.messages.get(id)?;
			let align = match msg.status {
				MessageStatus::Received => 0.0,
				_ => 1.0
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
							if let AttachmentData::FilePath(ref path) = att.data {
								/*gtk! { <Image file=path /> }*/
						        let pixbuf = Pixbuf::new_from_file(path).ok();
								gtk! { <Image pixbuf=pixbuf /> }
							} else {
								gtk! { <Label label="image data not found" /> }
							}
						} else {
							let text = format!("attachment of type {}", att.mime_type);
							gtk! { <Label label=text /> }
						}
					},
				}
			});
			Some(gtk! {
				<ListBoxRow>
				{widget_content}
				</ListBoxRow>
			})
		})
	}
}

#[derive(Clone, Default)]
struct ChatModelProps {
	chat: Chat,
}

impl Component for ChatModel {
	type Message = UiMessageChat;
	type Properties = ChatModelProps;

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		use UiMessageChat::*;
		match msg {
			NewMessage(id) => {
				self.chat_log.push(id);
				UpdateAction::Render
			},
			Send(items) => {
				let id = {
					let mut state = self.state.write().unwrap();
					let id = state.next_message_id;
					let num = state.my_number;
					state.messages.insert(id, MessageInfo {
						sender: num,
						time: 0,
						contents: items,
						status: MessageStatus::Sending,
					});
					state.next_message_id += 1;
					id
				};
				let fut = async move {
					NewMessage(id)
				};
				UpdateAction::Defer(std::boxed::Box::pin(fut))
			},
			AskDelete(_msg_id) => {
				UpdateAction::None
			},
			Delete(_msg_id) => {
				UpdateAction::Render
			},
			Nop => {
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<ChatModel> {
		let state = self.state.read().unwrap();
		gtk! {
			<Box::new(Orientation::Vertical, 0)>
				<ScrolledWindow Box::expand=true>
					<ListBox> //TODO: TreeView
					{self.generate_log_widgets(&*state)}
					</ListBox>
				</ScrolledWindow>
				<Box::new(Orientation::Horizontal, 0)>
					<Button label="" image="mail-attachment" always_show_image=true />
					<Entry
						Box::expand=true
						on realize=|entry| { entry.grab_focus(); UiMessageChat::Nop }
						on activate=|entry| {
							let text = entry.get_text().map(|x| x.to_string()).unwrap_or_default();
							let out = UiMessageChat::Send(vec![MessageItem::Text(text)]);
							entry.set_text("");
							out
						}
					/>
					<Button label="" image="go-next" always_show_image=true />
				</Box>
			</Box>
		}
	}
}
