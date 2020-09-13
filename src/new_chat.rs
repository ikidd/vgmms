use vgtk::lib::{glib, gtk::{*, Box as GtkBox}};
use vgtk::{gtk, Component, UpdateAction, VNode};

use std::default::Default;
use std::sync::{Arc, Mutex};

use crate::types::*;

#[derive(Clone, Debug, Default)]
pub struct NewChat {
	pub my_number: Number,
	pub my_country: Option<Country>,
	pub numbers: Vec<Number>,
	pub partial_num: String,
	pub numbers_shared: Arc<Mutex<Vec<Number>>>,
}

impl NewChat {
	fn num_addable(&self, num_str: &str) -> Option<Number> {
		let n = Number::normalize(num_str, self.my_country.unwrap())?;
		if !self.numbers.contains(&n) {
			Some(n)
		} else {
			None
		}
	}
}

#[derive(Clone, Debug)]
pub enum UiMessage {
	Add,
	Remove(usize),
	NumChanged(String),
	Nop,
}

/* find an ancestor of a widget with the given type */
fn find_ancestor<W: glib::IsA<Widget>, A: glib::IsA<Widget>>(w: &W) -> Option<A> {
	use glib::object::Cast;
	let mut w: Widget = w.clone().upcast();
	let mut count = 10;
	while let Some(parent) = w.get_parent() {
		w = parent;
		if let Some(_) = w.downcast_ref::<A>() {
			return w.downcast::<A>().ok()
		} else {
		}
		count -= 1;
		if count == 0 { break }
	}
	None
}

impl Component for NewChat {
	type Message = UiMessage;
	type Properties = Self;

	fn create(props: Self) -> Self {
		props
	}

	fn change(&mut self, props: Self) -> UpdateAction<Self> {
		*self = props;
		UpdateAction::Render
	}

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		use UiMessage::*;
		match msg {
			NumChanged(num) => {
				self.partial_num = num;
				UpdateAction::Render
			},
			Add => {
				if let Some(n) = self.num_addable(&*self.partial_num) {
					self.numbers.push(n);
					*self.numbers_shared.lock().unwrap() = self.numbers.clone();
					self.partial_num = String::new();
				}
				UpdateAction::Render
			},
			Remove(i) => {
				self.numbers.remove(i);
				*self.numbers_shared.lock().unwrap() = self.numbers.clone();
				UpdateAction::Render
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
		fn create_row(i: usize, num: Number) -> impl Iterator<Item=VNode<NewChat>> {
			use vgtk::ext::GridExtHelpers;
			vec![
			gtk! {
				<Label text=num.to_string()
					Grid::left=0 Grid::top={i as i32}
					xalign=1.0
					width_chars=12
				/>
			},
			gtk! {
				<Button::from_icon_name(Some("list-remove"), IconSize::Menu)
					Grid::left=1 Grid::top={i as i32}
					relief=ReliefStyle::None
					on clicked=|_| UiMessage::Remove(i) />
			},
			].into_iter()
		}
		let can_add = self.num_addable(&*self.partial_num).is_some();
		let can_open = self.numbers.len() > 0;
		use vgtk::ext::WindowExtHelpers;
		gtk! {
			/* we use with_buttons so we can pass flags, but we create our own buttons */
			<Dialog::with_buttons(Some("New Chat"), vgtk::current_window().as_ref(),
				DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
				&[])
				default_height=300
			>
				<GtkBox::new(Orientation::Vertical, 0)
					on parent_set=|w, _old| { set_expand_fill(w); UiMessage::Nop }
				>
					{
						let number_widgets = self.numbers.iter().enumerate()
							.flat_map(|(i, num)| create_row(i, *num));
						if self.numbers.len() > 0 { gtk! {
							<ScrolledWindow GtkBox::fill=true GtkBox::expand=true>
								<Grid halign=Align::Center>
								{number_widgets}
								</Grid>
							</ScrolledWindow>
						}} else { gtk! {
							<Box GtkBox::fill=true GtkBox::expand=true/>
						}}
					}
					<GtkBox::new(Orientation::Horizontal, 0)>
						<Entry
							GtkBox::expand=true
							text=self.partial_num.clone()
							input_purpose=InputPurpose::Phone
							on changed=|entry| {
								let text = entry.get_text().to_string();
								UiMessage::NumChanged(text)
							}
							on activate=|_| UiMessage::Add
							property_secondary_icon_name={if can_add {"list-add"} else {"input-dialpad"}}
							property_secondary_icon_activatable=can_add
							on icon_press=|_, _, _| UiMessage::Add
							on realize=|entry| { entry.grab_focus(); UiMessage::Nop }
						/>
					</GtkBox>
					/* buttons for the dialog */
					<GtkBox::new(Orientation::Horizontal, 0) homogeneous=true >
						<Button::from_icon_name(Some("gtk-cancel"), IconSize::Button) label="_Cancel" use_underline=true
							on clicked=|w| { find_ancestor::<_, Dialog>(w).unwrap().response(ResponseType::Cancel); UiMessage::Nop }
						/>
						<Button::from_icon_name(Some("gtk-open"), IconSize::Button) label="_Open" use_underline=true sensitive=can_open
							on clicked=|w| { find_ancestor::<_, Dialog>(w).unwrap().response(ResponseType::Accept); UiMessage::Nop }
						/>
					</GtkBox>
				</GtkBox>
			</Dialog>
		}
	}
}
