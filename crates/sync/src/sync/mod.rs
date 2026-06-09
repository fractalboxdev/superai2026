//! Rooms & sync (spec 01): the authoritative Loro relay (`serve`) and the
//! headless file-sync peer (`client`), plus the wire protocol and presence.

pub mod client;
pub mod presence;
pub mod protocol;
pub mod server;
