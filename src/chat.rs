use vgtk::ext::*;
use vgtk::lib::gtk::{self, *, Box as GtkBox};
use vgtk::{gtk, Component, UpdateAction, VNode};
use vgtk::lib::gdk_pixbuf::Pixbuf;
use vgtk::lib::{gio, glib};

use std::boxed::Box;
use std::default::Default;

use std::sync::{Arc, RwLock};
use crate::types::*;
use crate::db;
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
	Nop,
}

fn load_image(data: &[u8], width: i32, height: i32) -> Result<Pixbuf, glib::Error> {
	//TODO: reduce copying
	let data_stream = gio::MemoryInputStream::new_from_bytes(&glib::Bytes::from_owned(data.to_vec()));
	let pixbuf = Pixbuf::new_from_stream_at_scale(&data_stream,
		width, height, true, None::<&gio::Cancellable>);
	pixbuf
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
			let surf = pixbuf.create_surface(1, &window).unwrap();
			surf.set_device_scale(scale, scale);
			surf
		};
		gtk! { <Image property_surface=surf halign=halign /> }
	}
	#[cfg(not(surface))]
	gtk! { <Image pixbuf=Some(pixbuf) halign=halign /> }
}

impl ChatModel {
	fn generate_log_widgets<'a>(&'a self, state: &'a VgmmsState) -> impl Iterator<Item=VNode<Self>> + 'a {
		state.messages.iter().filter_map(move |(_id, msg)| {
			if msg.chat != self.chat.numbers {
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
						gtk! { <Label label=text selectable=true line_wrap=true line_wrap_mode=pango::WrapMode::WordChar xalign=align /> }
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
									let id = id.clone();
									gtk! { <EventBox on map=|eb| {
										let img_menu = gio::Menu::new();
										let item = gio::MenuItem::new(Some("_Save as..."), None);
										item.set_action_and_target_value(Some("app.save-attachment-dialog"), Some(&id.into()));
										img_menu.append_item(&item);

										let menu = Menu::new_from_model(&img_menu);
										set_long_press_rightclick_menu(eb, menu);
										UiMessageChat::Nop
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
								if let Err(e) = db::insert_attachment(&mut state.db_conn, &id, &att) {
									eprintln!("error saving attachment: {}", e);
								}
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
					let message = MessageInfo {
						sender: num,
						chat: self.chat.numbers.clone(),
						time: chrono::offset::Local::now().timestamp() as u64,
						contents: items,
						status: MessageStatus::Sending,
					};
					println!("inserting send {}: {:?}", hex::encode(&id[..]), message);
					match crate::dbus::send_message(&state.modem_path, &message, &state.attachments) {
						Ok(_) => (),
						Err(e) => eprintln!("error sending message: {}", e),
					};
					state.add_message(id.clone(), message);
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
			Nop => {
				UpdateAction::None
			},
		}
	}

	fn view(&self) -> VNode<ChatModel> {
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
				<ScrolledWindow GtkBox::expand=true on map=|sw| { keep_scrolled_to_bottom(sw); UiMessageChat::Nop} >
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
