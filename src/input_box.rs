use vgtk::lib::gtk::{self, *, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode, Callback};

use std::boxed::Box;
use std::default::Default;

use std::path::PathBuf;
use crate::types::*;

#[derive(Clone, Debug, Default)]
struct FileChooser {
	on_choose: Callback<Vec<PathBuf>>
}

#[derive(Clone, Debug)]
enum UiMessageFileChooser {
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
		}
		UpdateAction::None
	}

	fn view(&self) -> VNode<Self> {
		gtk! {
			<FileChooserDialog::with_buttons(Some("Select attachment"), None::<&gtk::Window>,
				FileChooserAction::Open,
				&[("_Cancel", ResponseType::Cancel), ("_Open", ResponseType::Accept)])
				on realize=|chooser| {chooser.set_select_multiple(true); UiMessageFileChooser::Nop}
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

fn once<A, F: FnOnce(A)>(f: F) -> impl Fn(A) {
    use std::cell::Cell;
    use std::rc::Rc;

    let f = Rc::new(Cell::new(Some(f)));
    move |value| {
        if let Some(f) = f.take() {
            f(value);
        } else {
            panic!("vgtk::once() function called twice 😒");
        }
    }
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
						size: size,
						data: AttachmentData::FileRef(path, 0, size),
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
						on_choose: {let cb: Callback<Vec<PathBuf>> = Box::new(once(move |filenames| {
							let _ = notify.send(filenames);
						})).into(); cb},
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
