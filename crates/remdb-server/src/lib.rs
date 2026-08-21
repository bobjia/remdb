//! remdb-server: a lean TCP query server for remdb over a protobuf protocol.
pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/remdb.rs"));
}

pub mod protocol;
pub mod buffer;
pub mod serialize;
pub mod handler;
pub mod server;
pub mod pubsub;