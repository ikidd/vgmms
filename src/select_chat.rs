use vgtk::lib::{glib, gtk::{*, Box as GtkBox}};
use vgtk::{gtk, Callback, Component, UpdateAction, VNode};

use std::default::Default;
use std::sync::{Arc, Mutex};

use crate::types::*;

#[derive(Clone, Default)]
pub struct SelectChat {
	pub my_number: Number,
	pub chats_summaries: Vec<(Chat, String)>,
	pub on_select: Callback<Vec<Number>>,
	pub on_new_chat: Callback<()>,
	pub numbers: Vec<Number>,
}

#[derive(Clone, Debug)]
pub enum UiMessageSelectChat {
	SelectionChanged(usize),
	NewChat,
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
				let nums = match self.chats_summaries.iter().nth(chat_idx) {
					Some((c, _summary)) => c.numbers.clone(),
					None => {
						eprintln!("selected chat could not be found!");
						return UpdateAction::Render
					},
				};
				self.numbers = nums;
				self.on_select.send(self.numbers.clone());
				UpdateAction::None
			},
			NewChat => {
				self.on_new_chat.send(());
				UpdateAction::None
			},
			Nop => {
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<Self> {
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
						<Image::from_icon_name(Some("mail-read"), IconSize::Menu) />
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
				<GtkBox::new(Orientation::Horizontal, 0)>
					<Button::from_icon_name(Some("add"), IconSize::Menu) on clicked=|_| UiMessageSelectChat::NewChat />
				</GtkBox>
				{
					let chat_widgets = self.chats_summaries.iter().map(
						|(c, desc)| create_chat_row(c, &desc, &self.my_number)
					);
					if self.chats_summaries.len() > 0 { gtk! {
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
	pub my_number: Number,
	pub chats_summaries: Vec<(Chat, String)>,
	pub numbers_shared: Arc<Mutex<Vec<Number>>>,
	pub on_new_chat: Callback<()>,
	pub numbers: Vec<Number>,
}

#[derive(Clone, Debug)]
pub enum UiMessageSelectChatDialog {
	Selected(Vec<Number>),
	NewChat,
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
			NewChat => {
				self.on_new_chat.send(());
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<Self> {
		use vgtk::ext::WindowExtHelpers;
		gtk! {
			<Dialog::with_buttons(Some("Select chat"), vgtk::current_window().as_ref(),
				DialogFlags::MODAL,
				&[("_Cancel", ResponseType::Cancel),
				("_Open", ResponseType::Accept)])
				default_height=300
			>
				<@SelectChat
					my_number=self.my_number.clone()
					chats_summaries=self.chats_summaries.clone()
					on select=|nums| {UiMessageSelectChatDialog::Selected(nums)}
					on new_chat=|_| {UiMessageSelectChatDialog::NewChat}
				/>
			</Dialog>
		}
	}
}
