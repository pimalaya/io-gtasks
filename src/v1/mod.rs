//! Google Tasks API v1.
//!
//! `rest` mirrors the REST resource tree (`tasklists`, `tasks`); the
//! sibling modules hold the transport primitive, the tri-state of the
//! writable fields, the query serializer and the optional std client.

#[cfg(feature = "std")]
pub mod client;
pub mod field;
#[cfg(feature = "client")]
pub mod query;
pub mod rest;
#[cfg(feature = "client")]
pub mod send;
