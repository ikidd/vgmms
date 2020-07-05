use vgtk::lib::{glib, gtk::{*, Box as GtkBox}};
use vgtk::{gtk, Callback, Component, UpdateAction, VNode};

use std::default::Default;
use std::sync::{Arc, Mutex, RwLock};

use crate::types::*;

#[derive(Clone, Default)]
pub struct SelectChat {
	pub state: Arc<RwLock<VgmmsState>>,
	pub on_select: Callback<Vec<Number>>,
	pub numbers: Vec<Number>,
}

#[derive(Clone, Debug)]
pub enum UiMessageSelectChat {
	SelectionChanged(usize),
	Nop,
}

impl Component for SelectChat {
	type Message = UiMessageSelectChat;
	type Properties = Self;

	fn create(props: Self) -> Self {
		props
	}

	fn change(&mut self, props: Self) -> UpdateAction<Self> {
		*self = props;
		UpdateAction::Render
	}

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		use UiMessageSelectChat::*;
		match msg {
			SelectionChanged(chat_idx) => {
				let state = self.state.read().unwrap();
				let nums = match state.chats.iter().nth(chat_idx) {
					Some(cm) => cm.0.numbers.clone(),
					None => {
						eprintln!("selected chat could not be found!");
						return UpdateAction::Render
					},
				};
				self.numbers = nums;
				self.on_select.send(self.numbers.clone());
				UpdateAction::None
			},
			Nop => {
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<Self> {
		let state = self.state.read().unwrap();
		fn summarize(msg_id: &MessageId, state: &VgmmsState) -> String {
			if let Some(msg) = state.messages.get(msg_id) {
				let mut summary = String::new();
				for item in &msg.contents {
					match item {
						MessageItem::Text(ref t) => {
							summary.push_str(t);
						},
						MessageItem::Attachment(ref id) => {
							match state.attachments.get(id) {
								Some(att) => summary.push_str(&format!("[attachment of type {}]", att.mime_type)),
								None => summary.push_str("[attachment {} not found]"),
							};
						},
					}
				}
				format!("[{}] {}: {}", msg.time, msg.sender.to_string(), summary)
			} else {
				"".into()
			}
		}
		fn set_expand_fill<P: glib::IsA<Widget>>(w: &P) {
			if let Some(p) = w.get_parent() {
				use glib::object::Cast;
				p.downcast_ref::<GtkBox>()
					.expect("not gtkbox")
					.set_child_packing(w, true, true, 0, PackType::Start);
			}
		}
		fn create_chat_row(c: &Chat, desc: &str, my_number: &Number) -> VNode<SelectChat> {
			let mut label_markup = "<b>".to_owned();
			label_markup.push_str(&glib::markup_escape_text(&c.get_name(my_number)));
			label_markup.push_str("</b>\n	<small>");
			label_markup.push_str(&glib::markup_escape_text(&desc));
			label_markup.push_str("</small>");
			gtk! {
				<ListBoxRow activatable=true>
					<GtkBox::new(Orientation::Horizontal, 3)>
						<Image::new_from_icon_name(Some("mail-read"), IconSize::Menu) />
						<Label text=label_markup
							use_markup=true
							xalign=0.0
							ellipsize=pango::EllipsizeMode::End
						/>
					</GtkBox>
				</ListBoxRow>
			}
		}
		gtk! {
			<GtkBox::new(Orientation::Vertical, 0)
				on parent_set=|w, _old| { set_expand_fill(w); UiMessageSelectChat::Nop }>
				{
					let chat_widgets = state.chats.iter().flat_map(|(c, info)| {
						let desc = if let Some((_tm, msg_id)) = info {
								summarize(msg_id, &state)
							} else {
								"".into()
							};
						create_chat_row(c, &desc, &state.my_number)
					});
					if state.chats.len() > 0 { gtk! {
						<ScrolledWindow GtkBox::fill=true GtkBox::expand=true>
							<ListBox
								on row_activated=|_box, row| UiMessageSelectChat::SelectionChanged(row.get_index() as usize)
							>
							{chat_widgets}
							</ListBox>
						</ScrolledWindow>
					} } else { gtk! {
						<Box GtkBox::fill=true GtkBox::expand=true/>
					} }
				}
			</GtkBox>
		}
	}
}



#[derive(Clone, Default)]
pub struct SelectChatDialog {
	pub state: Arc<RwLock<VgmmsState>>,
	pub numbers_shared: Arc<Mutex<Vec<Number>>>,
	pub numbers: Vec<Number>,
}

#[derive(Clone, Debug)]
pub enum UiMessageSelectChatDialog {
	Selected(Vec<Number>),
}

impl Component for SelectChatDialog {
	type Message = UiMessageSelectChatDialog;
	type Properties = Self;

	fn create(props: Self) -> Self {
		props
	}

	fn change(&mut self, props: Self) -> UpdateAction<Self> {
		*self = props;
		UpdateAction::Render
	}

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		use UiMessageSelectChatDialog::*;
		match msg {
			Selected(nums) => {
				*self.numbers_shared.lock().unwrap() = nums;
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<Self> {
		use vgtk::ext::WindowExtHelpers;
		gtk! {
			<Dialog::new_with_buttons(Some("Select chat"), vgtk::current_window().as_ref(),
				DialogFlags::MODAL,
				&[("_Cancel", ResponseType::Cancel),
				("_New chat", ResponseType::Other(0)),
				("_Select chat", ResponseType::Accept)])
				default_height=300
			>
				<@SelectChat
					state=self.state.clone()
					on select=|nums| {UiMessageSelectChatDialog::Selected(nums)}
				/>
			</Dialog>
		}
	}
}