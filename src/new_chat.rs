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
		if let Some(n) = Number::normalize(num_str, self.my_country.unwrap()) {
			if !self.numbers.contains(&n) {
				return Some(n)
			}
		}
		None
	}
}

#[derive(Clone, Debug)]
pub enum UiMessageNewChat {
	Add,
	Remove(usize),
	NumChanged(String),
	Accept,
	Cancel,
	Nop,
}

impl Component for NewChat {
	type Message = UiMessageNewChat;
	type Properties = Self;

	fn create(props: Self) -> Self {
		props
	}

	fn change(&mut self, props: Self) -> UpdateAction<Self> {
		*self = props;
		UpdateAction::Render
	}

	fn update(&mut self, msg: Self::Message) -> UpdateAction<Self> {
		use UiMessageNewChat::*;
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
				UpdateAction::Render
			},
			Accept => {
				UpdateAction::None
			}
			Cancel => {
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
				<Button::new_from_icon_name(Some("list-remove"), IconSize::Menu)
					Grid::left=1 Grid::top={i as i32}
					relief=ReliefStyle::None
					on clicked=|_| UiMessageNewChat::Remove(i) />
			},
			].into_iter()
		}
		let can_add = self.num_addable(&*self.partial_num).is_some();
		use vgtk::ext::WindowExtHelpers;
		gtk! {
			<Dialog::new_with_buttons(Some("New Chat"), vgtk::current_window().as_ref(),
				DialogFlags::MODAL | DialogFlags::DESTROY_WITH_PARENT,
				&[("_Cancel", ResponseType::Cancel), ("_Open", ResponseType::Accept)])
				default_height=300
				on response=|_d, resp| {if resp == ResponseType::Accept {
						UiMessageNewChat::Accept
					} else {
						UiMessageNewChat::Cancel}}
				>
				<GtkBox::new(Orientation::Vertical, 0)
					on parent_set=|w, _old| { set_expand_fill(w); UiMessageNewChat::Nop }>
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
								let text = entry.get_text().map(|x| x.to_string()).unwrap_or_default();
								UiMessageNewChat::NumChanged(text)
							}
							on activate=|_| UiMessageNewChat::Add
							property_secondary_icon_name={if can_add {"list-add"} else {"input-dialpad"}}
							property_secondary_icon_activatable=can_add
							on icon_press=|_, _, _| UiMessageNewChat::Add
							on realize=|entry| { entry.grab_focus(); UiMessageNewChat::Nop }
						/>
					</GtkBox>
				</GtkBox>
			</Dialog>
		}
	}
}
