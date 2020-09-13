use vgtk::ext::*;
use vgtk::lib::gtk::{self, *, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode};
use vgtk::lib::gdk_pixbuf::Pixbuf;
use vgtk::lib::{gio, glib};
use vgtk::Callback;

use std::default::Default;

use std::sync::{Arc, RwLock};
use crate::types::*;
use crate::input_box::InputBox;

#[derive(Clone, Default)]
pub struct ChatLog {
	pub state: Arc<RwLock<VgmmsState>>,
	pub on_send: Callback<(Chat, Vec<DraftItem>)>,
	pub chat: Chat,
}

#[derive(Clone, Debug)]
pub enum UiMessage {
	Send(Vec<DraftItem>),
	AskDelete(MessageId),
	Delete(MessageId),
	Nop,
}

fn load_image(data: &[u8], width: i32, height: i32) -> Result<Pixbuf, glib::Error> {
	let loader = gdk_pixbuf::PixbufLoader::new();
	use gdk_pixbuf::PixbufLoaderExt;
	/* if width and height are given, scale to keep resulting size below them */
	loader.connect_size_prepared(move |loader, actual_width, actual_height| {
		if width >= 0 && height >= 0 {
			let width_ratio = width as f64 / actual_width as f64;
			let height_ratio = height as f64 / actual_height as f64;
			let ratio = width_ratio.min(height_ratio);
			let new_width = (actual_width as f64 * ratio) as i32;
			let new_height = (actual_height as f64 * ratio) as i32;
			loader.set_size(new_width, new_height);
		}
	});
	loader.write(data)?;
	loader.close()?;
	match loader.get_pixbuf() {
		Some(p) => Ok(p),
		None => Err(glib::Error::new(gdk_pixbuf::PixbufError::Failed, "image could not be loaded"))
	}
}

fn set_long_press_rightclick_menu<P: gtk::prelude::IsA<Widget> + glib::value::SetValueOptional>(w: &P, menu: Menu) {
	use vgtk::lib::gtk::prelude::WidgetExtManual;
	use vgtk::lib::gdk;
	w.add_events(gdk::EventMask::BUTTON_PRESS_MASK|gdk::EventMask::TOUCH_MASK);
	menu.set_property_attach_widget(Some(w));

	let cb_menu = menu.clone();
	w.connect_button_press_event(move |_w, ev| {
		if ev.get_button() == 3 {
			cb_menu.popup_at_pointer(Some(&ev));
			return glib::signal::Inhibit(true);
		}
		glib::signal::Inhibit(false)
	});

	let w = w.clone();
	let gest = GestureLongPress::new(&w);
	gest.set_touch_only(true);
	gest.set_propagation_phase(PropagationPhase::Capture);
	gest.connect_cancelled(move |_gest| {
	});
	gest.connect_pressed(move |_gest, _x, _y| {
		if let Some(rect) = _gest.get_bounding_box() {
		if let Some(seq) = _gest.get_last_updated_sequence() {
		if let Some(ev) = _gest.get_last_event(Some(&seq)) {
			menu.popup_at_rect(&w.get_window().unwrap(), &rect,
				gdk::Gravity::NorthWest, gdk::Gravity::NorthWest,
				Some(&ev));
		}}}
	});
	//must leak gest to keep things working
	std::mem::forget(gest);
}

fn image_widget<T: Component>(pixbuf: Pixbuf, halign: gtk::Align) -> VNode<T> {
	#[cfg(surface)]
	{
		use vgtk::lib::gdk;
		let surf = {
			let max_width = 100f64;
			let width = pixbuf.get_width() as f64;
			let width_scale = width / max_width;
			let max_height = 100f64;
			let height = pixbuf.get_height() as f64;
			let height_scale = height / max_height;
			let scale = width_scale.max(height_scale);
			use gdk::prelude::{GdkPixbufExt, WindowExtManual};
			let window = gdk::Window::get_default_root_window();
			let surf = pixbuf.create_surface(1, Some(&window)).unwrap();
			surf.set_device_scale(scale, scale);
			surf
		};
		gtk! { <Image property_surface=surf halign=halign /> }
	}
	#[cfg(not(surface))]
	gtk! { <Image pixbuf=Some(pixbuf) halign=halign /> }
}

impl ChatLog {
	fn generate_log_widgets<'a>(&'a self, state: &'a VgmmsState) -> impl Iterator<Item=VNode<Self>> + 'a {
		state.messages.iter().filter_map(move |(msg_id, msg)| {
			if msg.chat != self.chat.numbers {
				return None
			}
			let (align, halign) = match msg.status {
				MessageStatus::Received => (0.0, gtk::Align::Start),
				_ => (1.0, gtk::Align::End),
			};
			use chrono::offset::TimeZone;
			let text = if let chrono::offset::LocalResult::Single(time) = chrono::Local.timestamp_opt(msg.time as i64, 0) {
				format!("[{}] {}", time.format("%k:%M"), msg.sender.to_string())
			} else {
				format!("[@{}] {}", msg.time, msg.sender.to_string())
			};
			let name_time = gtk! { <Label label=text selectable=true line_wrap=true line_wrap_mode=pango::WrapMode::WordChar xalign=align /> };
			let message_content = msg.contents.iter().map(move |item| {
				match item {
					MessageItem::Text(ref t) => {
						gtk! { <Label label=t.clone() selectable=true line_wrap=true line_wrap_mode=pango::WrapMode::WordChar xalign=align /> }
					},
					MessageItem::Attachment(id) => {
						let att = match state.attachments.get(&id) {
							Some(att) => att,
							None => {
								return gtk! { <Label label=format!("[attachment {} not found]", id) xalign=align /> }
							}
						};
						if att.mime_type.starts_with("image/") {
							match att.with_data(|data| {
									#[cfg(surface)]
									let dim = -1;
									#[cfg(not(surface))]
									let dim = 100;
									load_image(data, dim, dim)
								}) {
								Ok(Ok(pixbuf)) => {
									let image: VNode<Self> = image_widget(pixbuf, halign);
									let msg_id = msg_id.clone();
									let id = id.clone();
									gtk! { <EventBox on map=|eb| {
										let img_menu = gio::Menu::new();
										let item = gio::MenuItem::new(Some("_Save as..."), None);
										item.set_action_and_target_value(Some("app.save-attachment-dialog"), Some(&id.into()));
										img_menu.append_item(&item);
										let item = gio::MenuItem::new(Some("_Delete message"), None);
										item.set_action_and_target_value(Some("app.delete-message"), Some(&hex::encode(&msg_id[..]).into()));
										img_menu.append_item(&item);

										let menu = Menu::from_model(&img_menu);
										set_long_press_rightclick_menu(eb, menu);
										UiMessage::Nop
										}>{image}</EventBox>
									}
								},
								Ok(Err(e)) => gtk! { <Label label=format!("[image could not be loaded: {}]", e) xalign=align /> },
								Err(e) => {
									let path = &att.data.0;
									gtk! { <Label label=format!("[attachment at {} could not be opened: {}]", path.display(), e) xalign=align /> }
								},
							}
						} else {
							let text = format!("attachment of type {}", att.mime_type);
							gtk! { <Label label=text xalign=align /> }
						}
					},
				}
			});
			Some(gtk! {
				<ListBoxRow selectable=false>
					<GtkBox::new(Orientation::Vertical, 0)>
						{name_time}
						{message_content}
					</GtkBox>
				</ListBoxRow>
			})
		})
	}
}

impl Component for ChatLog {
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
			Send(draft_items) => {
				self.on_send.send((self.chat.clone(), draft_items));
				UpdateAction::Render
			}
			AskDelete(_msg_id) => {
				UpdateAction::None
			},
			Delete(_msg_id) => {
				UpdateAction::Render
			},
			Nop => {
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<ChatLog> {
		let state = self.state.read().unwrap();
		fn keep_scrolled_to_bottom(sw: &ScrolledWindow) {
			if let Some(adj) = sw.get_vadjustment() {
				adj.connect_property_upper_notify(|adj| {
					adj.set_value(adj.get_upper());
				});
			}
		}
		gtk! {
			<GtkBox::new(Orientation::Vertical, 0)>
				<ScrolledWindow GtkBox::expand=true on map=|sw| { keep_scrolled_to_bottom(sw); UiMessage::Nop} >
					<ListBox> //TODO: TreeView
					{self.generate_log_widgets(&*state)}
					</ListBox>
				</ScrolledWindow>
				<@InputBox
					on send=|draft| UiMessage::Send(draft)
				/>
			</GtkBox>
		}
	}
}
