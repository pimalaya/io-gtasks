#![no_std]
#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! # io-gtasks
//!
//! I/O-free coroutines for the [Google Tasks REST API], built on
//! [io-http] (HTTP/1.1) and pumped by any stream the caller owns.
//!
//! [Google Tasks REST API]: https://developers.google.com/workspace/tasks/reference/rest
//! [io-http]: https://docs.rs/io-http
//!
//! io-gtasks is the tasks sibling of [io-gmail] (mail), [io-gpeople]
//! (contacts) and [io-gcal] (calendar): same shape, same vendor, a
//! fourth Google API. The crate name carries the vendor because "tasks"
//! alone says nothing about who serves them.
//!
//! [io-gmail]: https://docs.rs/io-gmail
//! [io-gpeople]: https://docs.rs/io-gpeople
//! [io-gcal]: https://docs.rs/io-gcal
//!
//! ## Layers and features
//!
//! The crate has two of the three standard Pimalaya layers; there is no
//! CLI:
//!
//! 1. **I/O-free coroutines** (`no_std` core, always present): the whole
//!    Tasks REST logic.
//! 2. **Std client** ([`v1::client::GtasksClientStd`], `client`
//!    feature): a blocking pump over any stream, with `connect` opening
//!    the TCP/TLS connection itself behind a TLS feature (`rustls-ring`
//!    by default, `rustls-aws`, `native-tls`).
//!
//! The `ical` feature adds the iCalendar projection of a task onto a
//! VTODO and back, in [`v1::rest::tasks::ical`].
//!
//! ## Everything lives under v1
//!
//! The Tasks REST API is versioned (`/tasks/v1/`), so the crate is
//! too: the version-agnostic [`coroutine`] contract stays at the crate
//! root, everything else lives under [`v1`].
//!
//! ## The coroutine contract
//!
//! Every exchange implements [`coroutine::GtasksCoroutine`]: `resume`
//! takes the bytes read since the last yield and either requests I/O
//! ([`coroutine::GtasksYield`] `WantsRead` / `WantsWrite`) or
//! completes. The [`gtasks_try!`] macro is the coroutine equivalent of
//! `?`.
//!
//! A Tasks call is a single HTTP request/response, so every REST
//! coroutine is a thin wrapper around one shared primitive,
//! [`v1::send::GtasksSend`]: it builds the authorized request (bearer
//! token, JSON in and out) and parses either the 2xx body or the Tasks
//! error envelope into [`v1::send::GtasksSendError`]. Redirects are
//! never followed. The terminal [`v1::send::GtasksSendOutput`] carries
//! the parsed response plus a keep-alive flag so pumps can reuse the
//! connection across the many small requests a sync makes. Empty 2xx
//! bodies (delete, clear) deserialize into the
//! [`v1::send::GtasksNoResponse`] unit marker.
//!
//! ## Naming
//!
//! Public items follow `<Domain><Target><Verb><Ext>`: the domain is
//! `Gtasks`, the target-verb pair mirrors the REST method
//! ([`v1::rest::tasks::get::GtasksTaskGet`] for `tasks.get`) and the
//! extension distinguishes companions (`Params`, `Error`).
//!
//! A plural target marks a collection-level method and a singular one
//! an item-level method, which is what tells
//! [`v1::rest::tasks::list::GtasksTasksList`] (the tasks of a list)
//! from [`v1::rest::tasks::get::GtasksTaskGet`] (one of them). Pure data
//! resources omit the verb and keep the name the reference gives them:
//! [`v1::rest::tasks::GtasksTask`] and [`v1::rest::tasks::GtasksTasks`].
//! The stutter of `tasklists.list` is inherited rather than invented:
//! [`v1::rest::tasklists::list::GtasksTaskListsList`].
//!
//! ## Module layout
//!
//! [`v1::rest`] mirrors the Tasks REST reference one-to-one: each
//! resource is a directory and each method a file named after the API
//! method. Path parameters are percent-encoded segment by segment, so
//! an identifier never reads as a separator.
//!
//! Request bodies take the whole resource by reference
//! (`&GtasksTask`), and its writable fields are
//! [`v1::field::GtasksField`]s: unset fields stay out of the body, null
//! ones are sent as an explicit `null`, which is how a patch clears a
//! field rather than leaving it alone. List and move methods take
//! borrowed `*Params` structs flattened into query pairs by
//! [`v1::query::to_query_pairs`]; the insert method, carrying two
//! scalar parameters, takes them positionally instead.
//!
//! ## Incremental sync
//!
//! The Tasks API has neither a sync token nor a push channel. A
//! consumer keeping a local copy lists each task list with
//! `updatedMin` set to the greatest `updated` it last saw, plus
//! `showDeleted` and `showHidden`, so that deletions and the tasks
//! completed in another client come back as well. A deleted task stays
//! listable, flagged `deleted`; a task moved to another list keeps its
//! id and leaves nothing behind, so tasks are keyed by id across lists.
//!
//! ## Authentication
//!
//! io-gtasks does no OAuth itself: the API only accepts OAuth 2.0
//! bearer tokens, so the credential is exactly a bare access token, and
//! minting or refreshing it is the caller's responsibility.
//!
//! ## Logging
//!
//! Coroutines pair a `debug!` lifecycle line with one `trace!` per
//! input variable in `new()`, and a `debug!` plus `trace!("out: ...")`
//! when `resume` completes; the crate never logs above `debug!`.
//!
//! ## Example
//!
//! Running a coroutine against a caller-owned TLS stream:
//!
//! ```rust,no_run
//! use std::{
//!     io::{Read, Write},
//!     net::TcpStream,
//!     sync::Arc,
//! };
//!
//! use io_gtasks::{
//!     coroutine::*,
//!     v1::rest::tasks::list::{GtasksTasksList, GtasksTasksListParams},
//! };
//! use io_http::rfc6750::bearer::HttpAuthBearer;
//! use rustls::{ClientConfig, ClientConnection, StreamOwned};
//! use rustls_platform_verifier::ConfigVerifierExt;
//!
//! let config = ClientConfig::with_platform_verifier().unwrap();
//! let server_name = "tasks.googleapis.com".try_into().unwrap();
//! let conn = ClientConnection::new(Arc::new(config), server_name).unwrap();
//! let tcp = TcpStream::connect(("tasks.googleapis.com", 443)).unwrap();
//! let mut stream = StreamOwned::new(conn, tcp);
//!
//! let auth = HttpAuthBearer::new("token");
//! let params = GtasksTasksListParams {
//!     max_results: Some(100),
//!     ..Default::default()
//! };
//! let mut coroutine =
//!     GtasksTasksList::new(&auth, "@default", &params).unwrap();
//!
//! let mut arg: Option<&[u8]> = None;
//! let mut buf = [0u8; 8192];
//! let mut read = Vec::new();
//!
//! let out = loop {
//!     match coroutine.resume(arg.take()) {
//!         GtasksCoroutineState::Complete(Ok(out)) => break out,
//!         GtasksCoroutineState::Complete(Err(err)) => panic!("{err}"),
//!         GtasksCoroutineState::Yielded(GtasksYield::WantsRead) => {
//!             let n = stream.read(&mut buf).unwrap();
//!             read.clear();
//!             read.extend_from_slice(&buf[..n]);
//!             arg = Some(&read);
//!         }
//!         GtasksCoroutineState::Yielded(GtasksYield::WantsWrite(bytes)) => {
//!             stream.write_all(&bytes).unwrap();
//!         }
//!     }
//! };
//!
//! for task in &out.response.items {
//!     println!("{:?}: {:?}", task.due, task.title);
//! }
//! ```

extern crate alloc;
#[cfg(feature = "client")]
extern crate std;

pub mod coroutine;
pub mod v1;
