//! Replace a task (`tasks.update`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks/update>

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

/// I/O-free coroutine replacing a task (`tasks.update`).
pub struct GtasksTaskUpdate {
    send: GtasksSend<GtasksTask>,
}

impl GtasksTaskUpdate {
    /// Builds the `tasks.update` request, replacing the task of the
    /// given id as a whole: every writable field the task leaves unset
    /// is cleared.
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
        debug!("prepare task for update");
        trace!("tasklist_id: {tasklist_id:?}");
        trace!("task_id: {task_id:?}");
        trace!("task: {task:?}");

        let url = api_url(["lists", tasklist_id, "tasks", task_id])?;
        let send = GtasksSend::put_json_if_match(auth, url, task, if_match)?;

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskUpdate {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTask>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task updated");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
