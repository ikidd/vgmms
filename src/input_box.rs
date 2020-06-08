use vgtk::lib::gtk::{*, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode, Callback};

use std::boxed::Box;
use std::default::Default;

use std::path::PathBuf;

use crate::once;
use crate::types::*;

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
pub enum UiMessageFileChooser {
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
		};
		UpdateAction::None
	}

	fn view(&self) -> VNode<Self> {
		let (action, title, accept_label, select_multiple) = (self.action, self.title.clone(),
			self.accept_label.clone(), self.select_multiple);
		let name = self.default_name.clone();
		gtk! {
			<FileChooserDialog::with_buttons(Some(&*title), None::<&Window>,
				action.unwrap_or(FileChooserAction::Open),
				&[("_Cancel", ResponseType::Cancel), (&*accept_label, ResponseType::Accept)])
				select_multiple=select_multiple
				widget_name=name.unwrap_or("".into())

				on map=|chooser| {
					if let Some(name) = chooser.get_widget_name() {
						chooser.set_current_name(name.as_str())
					}; UiMessageFileChooser::Nop
				}
				on response=|chooser, _resp| UiMessageFileChooser::Choose(chooser.get_filenames())
			/>
		}
	}
}

#[derive(Clone, Default)]
pub struct InputBoxModel {
	pub file_paths: Vec<PathBuf>,
	pub message: String,
	pub on_send: Callback<Vec<DraftItem>>,
}

#[derive(Clone, Debug)]
pub enum UiMessageInputBox {
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
							eprintln!("could not stat file {}: {}", path.display(), e);
							continue
						},
					};
					let att = Attachment {
						name: filename,
						mime_type: tree_magic::from_filepath(&path),
						data: (path, 0, size),
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
						on_choose: {let cb: Callback<Vec<PathBuf>> = Box::new(once::once(move |filenames| {
							let _ = notify.send(filenames);
						})).into(); cb},
						action: Some(FileChooserAction::Open),
						title: "Select attachments".into(),
						select_multiple: true,
						accept_label: "_Open".into(),
						default_name: None,
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
