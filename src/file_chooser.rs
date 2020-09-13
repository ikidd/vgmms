use vgtk::lib::gtk::*;
use vgtk::{gtk, Component, UpdateAction, VNode, Callback};

use std::default::Default;

use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct FileChooser {
	pub on_choose: Callback<Vec<PathBuf>>,
	pub action: Option<FileChooserAction>,
	pub title: String,
	pub select_multiple: bool,
	pub accept_label: String,
	pub default_name: Option<String>,
}

#[derive(Clone, Debug)]
pub enum UiMessage {
	Choose(Vec<PathBuf>),
}

impl Component for FileChooser {
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
		match msg {
		    UiMessage::Choose(fns) => self.on_choose.send(fns),
		}
		UpdateAction::None
	}

	fn view(&self) -> VNode<Self> {
		let (action, title, accept_label, select_multiple) = (self.action, self.title.clone(),
			self.accept_label.clone(), self.select_multiple);
		let name = self.default_name.clone();
		use crate::new_custom::NewCustom;
		gtk! {
			<FileChooserDialog::new_custom(|| {
					let chooser = FileChooserDialog::with_buttons(Some(&*title), None::<&Window>,
					action.unwrap_or(FileChooserAction::Open),
					&[("_Cancel", ResponseType::Cancel), (&*accept_label, ResponseType::Accept)]);
					if let Some(ref name) = &name {
						chooser.set_current_name(name.as_str());
					}
					chooser
				})
				select_multiple=select_multiple
				on response=|chooser, _resp| UiMessage::Choose(chooser.get_filenames())
			/>
		}
	}
}
