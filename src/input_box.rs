use vgtk::lib::gtk::{*, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode, Callback};

use std::boxed::Box;
use std::default::Default;

use std::path::PathBuf;

use crate::file_chooser;
use crate::once;
use crate::types::*;

#[derive(Clone, Default)]
pub struct InputBox {
	pub file_paths: Vec<PathBuf>,
	pub message: String,
	pub on_send: Callback<Vec<DraftItem>>,
	pub message_typed: bool,
}

#[derive(Clone, Debug)]
pub enum UiMessage {
	Send,
	TextChanged(String),
	ToggleFile,
	AskForFile,
	SetFiles(Vec<PathBuf>),
	Clear,
	Nop,
}

impl Component for InputBox {
	type Message = UiMessage;
	type Properties = Self;

	fn create(props: Self) -> Self {
		props
	}

	fn change(&mut self, mut props: Self) -> UpdateAction<Self> {
		/* preserve message/attachments if self.message_typed */
		if self.message_typed {
			std::mem::swap(&mut props.file_paths, &mut self.file_paths);
			std::mem::swap(&mut props.message, &mut self.message);
			props.message_typed = true;
		}
		*self = props;
		UpdateAction::Render
	}

	fn update(&mut self, mut msg: Self::Message) -> UpdateAction<Self> {
		use UiMessage::*;
		if let ToggleFile = msg {
			msg = if self.file_paths.len() == 0 { UiMessage::AskForFile } else { UiMessage::Clear };
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
				self.message_typed = true;
				UpdateAction::None
			},
			AskForFile => {
				let (notify, fns_result) = futures::channel::oneshot::channel();

				let fut = vgtk::run_dialog_props::<file_chooser::FileChooser>(vgtk::current_window().as_ref(),
					file_chooser::FileChooser {
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
			SetFiles(paths) => {
				self.file_paths = paths;
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
					on clicked=|_entry| UiMessage::AskForFile
				/>*/
				/*<Button label="" image="edit-clear" /*visible={self.file_paths.len() > 0}*/ always_show_image=true
					on clicked=|_entry| UiMessage::ClearFiles
				/>*/
				<Entry
					text=self.message.clone()
					GtkBox::expand=true
					property_secondary_icon_name={if files_empty { "mail-attachment" } else { "edit-clear" }}
					on icon_press=|_entry, _pos, _ev| UiMessage::ToggleFile
					on realize=|entry| { entry.grab_focus(); UiMessage::Nop }
					on changed=|entry| {
						let text = entry.get_text().to_string();
						UiMessage::TextChanged(text)
					}
					on activate=|_| UiMessage::Send
				/>
				<Button label="" image="go-next" always_show_image=true
					on clicked=|_| UiMessage::Send
				/>
			</GtkBox>
		}
	}
}
