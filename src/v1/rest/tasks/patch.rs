//! Patch a task (`tasks.patch`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks/patch>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gtasks_try,
    v1::{
        rest::tasks::GtasksTask,
        send::{GtasksSend, GtasksSendError, GtasksSendOutput, api_url},
    },
};

/// I/O-free coroutine patching a task (`tasks.patch`).
pub struct GtasksTaskPatch {
    send: GtasksSend<GtasksTask>,
}

impl GtasksTaskPatch {
    /// Builds the `tasks.patch` request: a set field is replaced, a null
    /// one cleared and an unset one left as the server holds it.
    ///
    /// `if_match` gates the write on the etag a read returned, so a
    /// concurrent change comes back as HTTP 412 rather than being
    /// overwritten; `None` overwrites unconditionally.
    pub fn new(
        auth: &HttpAuthBearer,
        tasklist_id: &str,
        task_id: &str,
        task: &GtasksTask,
        if_match: Option<&str>,
    ) -> Result<Self, GtasksSendError> {
        debug!("prepare task for patch");
        trace!("tasklist_id: {tasklist_id:?}");
        trace!("task_id: {task_id:?}");
        trace!("task: {task:?}");

        let url = api_url(["lists", tasklist_id, "tasks", task_id])?;
        let send = GtasksSend::patch_json_if_match(auth, url, task, if_match)?;

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskPatch {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTask>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task patched");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
