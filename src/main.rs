#![recursion_limit="512"]

use vgtk::ext::*;
use vgtk::lib::gio::ApplicationFlags;
use vgtk::lib::gtk::*;
use vgtk::{gtk, run, Component, UpdateAction, VNode};

use std::default::Default;
use std::sync::{Arc, Mutex};

#[derive(Copy, Clone, Debug, Default)]
struct Number {
	num: u64,
}

impl Number {
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

#[derive(Clone, Debug, Default)]
struct Model {}

#[derive(Clone, Debug)]
enum UiMessageTriv {
	Exit,
}

/**
  attachments are owned by the message that contains them
*/
struct Attachment {
	filename: Vec<u8>,
	mimetype: Vec<u8>,
	contents: Vec<u8>,
}

type AttachmentId = usize;
type MessageId = usize;

#[derive(Clone, Debug)]
enum MessageItem {
	Text(String),
	AttachmentId(usize),
}

#[derive(Clone, Debug)]
struct Contact {
	number: Number,
	name: String,
}

#[derive(Clone, Debug)]
struct MessageInfo {
	sender: Number,
	time: u64,
	contents: Vec<MessageItem>,
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
		VgmmsState {
			chats: Default::default(),
			messages: Default::default(),
			contacts: Default::default(),
			attachments: Default::default(),
			next_message_id: 4321,
			next_attachment_id: 1,
			my_number: Number { num: 4561237890 },
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
						<Label label="vgmms" />
						<Notebook Box::expand=true>
							<@ChatModel />
						</Notebook>
						<Label label="vgmms" />
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
	state: Arc<Mutex<VgmmsState>>,
	target: Chat,
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

impl Component for ChatModel {
	type Message = UiMessageChat;
	type Properties = ();

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		use UiMessageChat::*;
		match msg {
			NewMessage(id) => {
				self.chat_log.push(id);
				UpdateAction::Render
			},
			Send(items) => {
				let id = {
					let mut state = self.state.lock().unwrap();
					let id = state.next_message_id;
					let num = state.my_number;
					state.messages.insert(id, MessageInfo {
						sender: num,
						time: 0,
						contents: items,
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
				UpdateAction::None
			},
			Nop => {
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<ChatModel> {
		let state = self.state.lock().unwrap();
		gtk! {
			<Box::new(Orientation::Vertical, 0)>
				<ScrolledWindow Box::expand=true>
					<ListBox> //TODO: TreeView
						{ self.chat_log.iter().filter_map(|id| {
							let msg = state.messages.get(id)?;
							let contents: String = match msg.contents.iter().next() {
								Some(&MessageItem::Text(ref t)) => t.into(),
								_ => "other".into(),
							};
							let text = format!("[{}] {}: {}", msg.time, msg.sender.num, contents);
							Some(gtk! {
								<ListBoxRow>
								<Label label=text line_wrap=true line_wrap_mode=pango::WrapMode::WordChar xalign=0.0 />
								</ListBoxRow>
							})
						})}
					</ListBox>
				</ScrolledWindow>
				<Entry
					on realize=|entry| { entry.grab_focus(); UiMessageChat::Nop }
					on activate=|entry| {
						let text = entry.get_text().map(|x| x.to_string()).unwrap_or_default();
						let out = UiMessageChat::Send(vec![MessageItem::Text(text)]);
						entry.set_text("");
						out
					}
				/>
			</Box>
		}
	}
}
