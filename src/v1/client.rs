//! Std-blocking Google Tasks client, gated behind the `client` feature.
//!
//! Wraps a `Read + Write` stream plus the bearer credential and runs
//! the coroutines against `tasks.googleapis.com`.

#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use core::time::Duration;
use core::{any::Any, fmt};

#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use alloc::string::String;
use alloc::{boxed::Box, string::ToString};
use std::io::{self, Read, Write};

use io_http::rfc6750::bearer::HttpAuthBearer;
#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use pimalaya_stream::{
    stream::{Stream, TcpConnectOptions, TlsConnectOptions},
    tls::Tls,
};
use thiserror::Error;
#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use url::Url;

#[cfg(any(
    feature = "rustls-aws",
    feature = "rustls-ring",
    feature = "native-tls"
))]
use crate::v1::send::GTASKS_API_BASE;
use crate::{
    coroutine::*,
    v1::{
        rest::{
            tasklists::{
                GtasksTaskList, GtasksTaskLists,
                delete::GtasksTaskListDelete,
                get::GtasksTaskListGet,
                insert::GtasksTaskListInsert,
                list::{GtasksTaskListsList, GtasksTaskListsListParams},
                patch::GtasksTaskListPatch,
                update::GtasksTaskListUpdate,
            },
            tasks::{
                GtasksTask, GtasksTasks,
                clear::GtasksTasksClear,
                delete::GtasksTaskDelete,
                get::GtasksTaskGet,
                insert::GtasksTaskInsert,
                list::{GtasksTasksList, GtasksTasksListParams},
                r#move::{GtasksTaskMove, GtasksTaskMoveParams},
                patch::GtasksTaskPatch,
                update::GtasksTaskUpdate,
            },
        },
        send::{GtasksNoResponse, GtasksSendError, GtasksSendOutput},
    },
};

/// Errors that can occur on the std client.
#[derive(Debug, Error)]
pub enum GtasksClientStdError {
    /// The Tasks exchange itself failed.
    #[error(transparent)]
    Send(#[from] GtasksSendError),
    /// Reading from or writing to the stream failed.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Opening the TCP/TLS connection failed.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    #[error(transparent)]
    Tls(#[from] anyhow::Error),
    /// The API base URL carries no host to connect to.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    #[error("Tasks URL `{0}` has no host")]
    UrlMissingHost(String),
    /// The API base URL scheme is neither http nor https.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    #[error("Tasks URL `{url}` has unsupported scheme `{scheme}` (expected `http` or `https`)")]
    UrlUnsupportedScheme {
        /// The offending URL.
        url: String,
        /// The unsupported scheme it carries.
        scheme: String,
    },
}

/// Optional settings for [`GtasksClientStd::connect`]; the TLS backend
/// default is the only one there is.
#[derive(Debug, Default)]
pub struct GtasksClientStdConnectOptions {
    /// TLS backend configuration.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    pub tls: Tls,
}

const READ_BUFFER_SIZE: usize = 16 * 1024;

/// Standard, blocking Google Tasks client.
///
/// Owns the stream and the bearer credential; each convenience method
/// builds the matching coroutine and runs it to completion. Every task
/// list is addressed explicitly by id, `@default` being the alias of
/// the default list of the user.
pub struct GtasksClientStd {
    /// The underlying TCP or TLS stream.
    pub stream: Box<dyn GtasksStream>,
    /// The OAuth 2.0 bearer credential added to every request.
    pub auth: HttpAuthBearer,
}

impl GtasksClientStd {
    /// Builds a client over an already-connected stream.
    pub fn new<S: Read + Write + Send + 'static>(stream: S, token: impl ToString) -> Self {
        Self {
            stream: Box::new(stream),
            auth: HttpAuthBearer::new(token.to_string()),
        }
    }

    /// Opens a TCP/TLS connection to `tasks.googleapis.com` and builds
    /// the client around it.
    #[cfg(any(
        feature = "rustls-aws",
        feature = "rustls-ring",
        feature = "native-tls"
    ))]
    pub fn connect(
        token: impl ToString,
        options: GtasksClientStdConnectOptions,
    ) -> Result<Self, GtasksClientStdError> {
        let GtasksClientStdConnectOptions { tls } = options;

        let url = Url::parse(GTASKS_API_BASE).expect("Tasks API base URL is valid");
        let host = url
            .host_str()
            .ok_or_else(|| GtasksClientStdError::UrlMissingHost(url.to_string()))?;

        let stream = match url.scheme() {
            "http" => {
                let port = url.port().unwrap_or(80);
                Stream::connect_tcp(host, port, TcpConnectOptions::default())?
            }
            "https" => {
                let port = url.port().unwrap_or(443);
                let opts = TlsConnectOptions {
                    tls: tls.clone(),
                    ..Default::default()
                };

                Stream::connect_tls(host, port, opts)?
            }
            scheme => {
                return Err(GtasksClientStdError::UrlUnsupportedScheme {
                    url: url.to_string(),
                    scheme: scheme.to_string(),
                });
            }
        };

        stream.set_read_timeout(Some(Duration::from_secs(30)))?;

        Ok(Self {
            stream: Box::new(stream),
            auth: HttpAuthBearer::new(token.to_string()),
        })
    }

    /// Replaces the underlying stream, e.g. after reconnecting.
    pub fn set_stream<S: Read + Write + Send + 'static>(&mut self, stream: S) {
        self.stream = Box::new(stream);
    }

    /// Runs the given coroutine to completion against the stream,
    /// reading on `WantsRead` and writing on `WantsWrite`.
    pub fn run<C, T>(
        &mut self,
        mut coroutine: C,
    ) -> Result<GtasksSendOutput<T>, GtasksClientStdError>
    where
        C: GtasksCoroutine<
                Yield = GtasksYield,
                Return = Result<GtasksSendOutput<T>, GtasksSendError>,
            >,
    {
        let mut buf = [0u8; READ_BUFFER_SIZE];
        let mut arg: Option<&[u8]> = None;

        loop {
            match coroutine.resume(arg.take()) {
                GtasksCoroutineState::Complete(Ok(out)) => return Ok(out),
                GtasksCoroutineState::Complete(Err(err)) => return Err(err.into()),
                GtasksCoroutineState::Yielded(GtasksYield::WantsRead) => {
                    let n = self.stream.read(&mut buf)?;
                    arg = Some(&buf[..n]);
                }
                GtasksCoroutineState::Yielded(GtasksYield::WantsWrite(bytes)) => {
                    self.stream.write_all(&bytes)?;
                    arg = None;
                }
            }
        }
    }

    /// Lists the task lists of the user (`tasklists.list`).
    pub fn tasklists_list(
        &mut self,
        params: &GtasksTaskListsListParams,
    ) -> Result<GtasksSendOutput<GtasksTaskLists>, GtasksClientStdError> {
        let coroutine = GtasksTaskListsList::new(&self.auth, params)?;
        self.run(coroutine)
    }

    /// Gets a task list by id (`tasklists.get`).
    pub fn tasklist_get(
        &mut self,
        tasklist_id: &str,
    ) -> Result<GtasksSendOutput<GtasksTaskList>, GtasksClientStdError> {
        let coroutine = GtasksTaskListGet::new(&self.auth, tasklist_id)?;
        self.run(coroutine)
    }

    /// Creates a task list (`tasklists.insert`).
    pub fn tasklist_insert(
        &mut self,
        tasklist: &GtasksTaskList,
    ) -> Result<GtasksSendOutput<GtasksTaskList>, GtasksClientStdError> {
        let coroutine = GtasksTaskListInsert::new(&self.auth, tasklist)?;
        self.run(coroutine)
    }

    /// Replaces a task list (`tasklists.update`).
    pub fn tasklist_update(
        &mut self,
        tasklist_id: &str,
        tasklist: &GtasksTaskList,
    ) -> Result<GtasksSendOutput<GtasksTaskList>, GtasksClientStdError> {
        let coroutine = GtasksTaskListUpdate::new(&self.auth, tasklist_id, tasklist)?;
        self.run(coroutine)
    }

    /// Patches a task list (`tasklists.patch`).
    pub fn tasklist_patch(
        &mut self,
        tasklist_id: &str,
        tasklist: &GtasksTaskList,
    ) -> Result<GtasksSendOutput<GtasksTaskList>, GtasksClientStdError> {
        let coroutine = GtasksTaskListPatch::new(&self.auth, tasklist_id, tasklist)?;
        self.run(coroutine)
    }

    /// Deletes a task list (`tasklists.delete`).
    pub fn tasklist_delete(
        &mut self,
        tasklist_id: &str,
    ) -> Result<GtasksSendOutput<GtasksNoResponse>, GtasksClientStdError> {
        let coroutine = GtasksTaskListDelete::new(&self.auth, tasklist_id)?;
        self.run(coroutine)
    }

    /// Lists the tasks of a task list (`tasks.list`).
    pub fn tasks_list(
        &mut self,
        tasklist_id: &str,
        params: &GtasksTasksListParams,
    ) -> Result<GtasksSendOutput<GtasksTasks>, GtasksClientStdError> {
        let coroutine = GtasksTasksList::new(&self.auth, tasklist_id, params)?;
        self.run(coroutine)
    }

    /// Gets a task by id (`tasks.get`).
    pub fn task_get(
        &mut self,
        tasklist_id: &str,
        task_id: &str,
    ) -> Result<GtasksSendOutput<GtasksTask>, GtasksClientStdError> {
        let coroutine = GtasksTaskGet::new(&self.auth, tasklist_id, task_id)?;
        self.run(coroutine)
    }

    /// Creates a task (`tasks.insert`), optionally under a parent and
    /// after a sibling.
    pub fn task_insert(
        &mut self,
        tasklist_id: &str,
        task: &GtasksTask,
        parent: Option<&str>,
        previous: Option<&str>,
    ) -> Result<GtasksSendOutput<GtasksTask>, GtasksClientStdError> {
        let coroutine = GtasksTaskInsert::new(&self.auth, tasklist_id, task, parent, previous)?;
        self.run(coroutine)
    }

    /// Replaces a task (`tasks.update`), optionally guarded by the etag
    /// a read returned.
    pub fn task_update(
        &mut self,
        tasklist_id: &str,
        task_id: &str,
        task: &GtasksTask,
        if_match: Option<&str>,
    ) -> Result<GtasksSendOutput<GtasksTask>, GtasksClientStdError> {
        let coroutine = GtasksTaskUpdate::new(&self.auth, tasklist_id, task_id, task, if_match)?;
        self.run(coroutine)
    }

    /// Patches a task (`tasks.patch`), optionally guarded by the etag a
    /// read returned.
    pub fn task_patch(
        &mut self,
        tasklist_id: &str,
        task_id: &str,
        task: &GtasksTask,
        if_match: Option<&str>,
    ) -> Result<GtasksSendOutput<GtasksTask>, GtasksClientStdError> {
        let coroutine = GtasksTaskPatch::new(&self.auth, tasklist_id, task_id, task, if_match)?;
        self.run(coroutine)
    }

    /// Deletes a task (`tasks.delete`), optionally guarded by the etag a
    /// read returned.
    pub fn task_delete(
        &mut self,
        tasklist_id: &str,
        task_id: &str,
        if_match: Option<&str>,
    ) -> Result<GtasksSendOutput<GtasksNoResponse>, GtasksClientStdError> {
        let coroutine = GtasksTaskDelete::new(&self.auth, tasklist_id, task_id, if_match)?;
        self.run(coroutine)
    }

    /// Moves a task under another parent, after another sibling or to
    /// another list (`tasks.move`).
    pub fn task_move(
        &mut self,
        tasklist_id: &str,
        task_id: &str,
        params: &GtasksTaskMoveParams,
    ) -> Result<GtasksSendOutput<GtasksTask>, GtasksClientStdError> {
        let coroutine = GtasksTaskMove::new(&self.auth, tasklist_id, task_id, params)?;
        self.run(coroutine)
    }

    /// Hides the completed tasks of a task list (`tasks.clear`).
    pub fn tasks_clear(
        &mut self,
        tasklist_id: &str,
    ) -> Result<GtasksSendOutput<GtasksNoResponse>, GtasksClientStdError> {
        let coroutine = GtasksTasksClear::new(&self.auth, tasklist_id)?;
        self.run(coroutine)
    }
}

impl fmt::Debug for GtasksClientStd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GtasksClientStd")
            .field("auth", &self.auth)
            .finish_non_exhaustive()
    }
}

/// Boxable client stream: `Read + Write + Send` plus `Any` so callers
/// can downcast back to the concrete stream type.
pub trait GtasksStream: Read + Write + Send + Any {
    /// Returns the stream as a mutable `Any` for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Read + Write + Send + Any> GtasksStream for T {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
