// This code was autogenerated with `dbus-codegen-rust -g -m None -d org.ofono.mms -f org.ofono.mms.Manager -p /org/ofono/mms`, see https://github.com/diwic/dbus-rs
use dbus as dbus;
use dbus::arg;
use dbus::blocking;

pub trait OrgOfonoMmsManager {
    fn get_services(&self) -> Result<Vec<(dbus::Path<'static>, ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg + 'static>>>)>, dbus::Error>;
}

impl<'a, C: ::std::ops::Deref<Target=blocking::Connection>> OrgOfonoMmsManager for blocking::Proxy<'a, C> {

    fn get_services(&self) -> Result<Vec<(dbus::Path<'static>, ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg + 'static>>>)>, dbus::Error> {
        self.method_call("org.ofono.mms.Manager", "GetServices", ())
            .and_then(|r: (Vec<(dbus::Path<'static>, ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg + 'static>>>)>, )| Ok(r.0, ))
    }
}

#[derive(Debug)]
pub struct OrgOfonoMmsManagerServiceAdded {
    pub path: dbus::Path<'static>,
    pub properties: ::std::collections::HashMap<String, arg::Variant<Box<dyn arg::RefArg + 'static>>>,
}

impl arg::AppendAll for OrgOfonoMmsManagerServiceAdded {
    fn append(&self, i: &mut arg::IterAppend) {
        arg::RefArg::append(&self.path, i);
        arg::RefArg::append(&self.properties, i);
    }
}

impl arg::ReadAll for OrgOfonoMmsManagerServiceAdded {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(OrgOfonoMmsManagerServiceAdded {
            path: i.read()?,
            properties: i.read()?,
        })
    }
}

impl dbus::message::SignalArgs for OrgOfonoMmsManagerServiceAdded {
    const NAME: &'static str = "ServiceAdded";
    const INTERFACE: &'static str = "org.ofono.mms.Manager";
}

#[derive(Debug)]
pub struct OrgOfonoMmsManagerServiceRemoved {
    pub path: dbus::Path<'static>,
}

impl arg::AppendAll for OrgOfonoMmsManagerServiceRemoved {
    fn append(&self, i: &mut arg::IterAppend) {
        arg::RefArg::append(&self.path, i);
    }
}

impl arg::ReadAll for OrgOfonoMmsManagerServiceRemoved {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(OrgOfonoMmsManagerServiceRemoved {
            path: i.read()?,
        })
    }
}

impl dbus::message::SignalArgs for OrgOfonoMmsManagerServiceRemoved {
    const NAME: &'static str = "ServiceRemoved";
    const INTERFACE: &'static str = "org.ofono.mms.Manager";
}
