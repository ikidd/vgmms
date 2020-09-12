#![recursion_limit="1024"]

#[macro_use]
extern crate lazy_static;

use vgtk::lib::gio;
use vgtk::lib::gtk::*;
use vgtk::lib::glib;

/* widgets */
mod chat;
mod file_chooser;
mod input_box;
mod new_chat;
mod select_chat;
mod window;

/* logic */
mod new_custom;
mod once;
mod types;
mod smil;
mod state;

/* persistence */
mod db;

/* dbus interfaces */
mod dbus;
mod mmsd_manager;
mod mmsd_service;
mod ofono_manager;
mod ofono_simmanager;

use window::*;

fn main() {
	use gio::prelude::ApplicationExtManual;
	use gio::ApplicationExt;
	use futures::stream::StreamExt;

	let args = &*std::env::args().collect::<Vec<_>>();

	/* receive notifications of new/updated SMS and MMS messages from DBus */
	let notif_stream = dbus::start_recv();
	pretty_env_logger::init();
	let (app, scope) = vgtk::start::<WindowModel>();
	let scope_ = scope.clone();
	std::thread::spawn(
		move || futures::executor::block_on(
			notif_stream.for_each(move |notif| {
				println!("notif sent!");
				scope_.try_send(window::UiMessage::Notif(notif)).unwrap();
				futures::future::ready(())
		}))
	);

	/* add options */
	app.add_main_option("daemon", glib::Char::new('d').unwrap(), glib::OptionFlags::NONE, glib::OptionArg::None,
		"run in the background without opening a window",
		None);
	
	/* do we need to hide the newly-created window? */
	let daemon = std::rc::Rc::new(std::cell::RefCell::new(false));

	/* handle command-line arguments */
	let daemon_ = daemon.clone();
	app.connect_handle_local_options(move |app, args_dict| {
		if let Some(daemon_arg) = args_dict.lookup_value("daemon", Some(glib::VariantTy::new("b").unwrap())) {
			if daemon_arg.get::<bool>().unwrap() {
				if app.get_is_remote() {
					std::process::exit(0);
				}
				daemon_.replace(true);
			}
		}
		-1
	});

	/* present the window when the application is remotely activated */
	let daemon_ = daemon.clone();
	app.connect_activate(move |app| {
		if let Some(w) = app.get_active_window() {
			/* the first activate is from the program starting;
			if we're in daemon mode, we should hide the window now */
			if !daemon_.replace(false) {
				w.present_with_time(0);
			} else {
				w.hide();
			}
		}
	});

	if !app.get_is_remote() {
		app.hold();
	}
	std::process::exit(app.run(args));
}
