use vgtk::ext::*;
use vgtk::lib::gtk::{self, *, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode};
use vgtk::lib::gdk_pixbuf::Pixbuf;

use std::boxed::Box;
use std::default::Default;

use std::sync::{Arc, RwLock};
use crate::types::*;

use crate::input_box::*;

#[derive(Clone, Default)]
pub struct ChatModel {
	pub state: Arc<RwLock<VgmmsState>>,
	pub chat_log: Vec<MessageId>,
	pub chat: Chat,
}

#[derive(Clone, Debug)]
pub enum UiMessageChat {
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
						let text = format!("[{}] {}: {}", msg.time, msg.sender.to_string(), t);
						gtk! { <Label label=text line_wrap=true line_wrap_mode=pango::WrapMode::WordChar xalign=align /> }
					},
					MessageItem::Attachment(ref id) => {
						let att = state.attachments.get(id).expect("attachment not found!");
						if att.mime_type.starts_with("image/") {
							let AttachmentData::FileRef(ref path, start, len) = att.data;
							if let Ok(pixbuf) = Pixbuf::new_from_file_at_size(path, 200, 200) {
								gtk! { <Image pixbuf=Some(pixbuf) halign=halign /> }
							} else {
								gtk! { <Label label="unloadable image" xalign=align /> }
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
