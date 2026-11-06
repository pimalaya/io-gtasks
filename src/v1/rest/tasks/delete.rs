//! Delete a task (`tasks.delete`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks/delete>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gtasks_try,
    v1::send::{GtasksNoResponse, GtasksSend, GtasksSendError, GtasksSendOutput, api_url},
};

/// I/O-free coroutine deleting a task (`tasks.delete`).
pub struct GtasksTaskDelete {
    send: GtasksSend<GtasksNoResponse>,
}

impl GtasksTaskDelete {
    /// Builds the `tasks.delete` request for the given task list and
    /// task id.
    ///
    /// The task is flagged deleted rather than erased, so a listing
    /// with `showDeleted` still returns it, and its subtasks are deleted
    /// with it. Deleting an assigned task also deletes the original one
    /// on the surface it comes from.
    ///
    /// `if_match` gates the deletion on the etag a read returned, so a
    /// task that changed since comes back as HTTP 412 rather than
    /// vanishing.
    pub fn new(
        auth: &HttpAuthBearer,
        tasklist_id: &str,
        task_id: &str,
        if_match: Option<&str>,
    ) -> Result<Self, GtasksSendError> {
        debug!("prepare task deletion");
        trace!("tasklist_id: {tasklist_id:?}");
        trace!("task_id: {task_id:?}");

        let url = api_url(["lists", tasklist_id, "tasks", task_id])?;
        let send = GtasksSend::delete_if_match(auth, url, if_match);

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskDelete {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksNoResponse>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task deleted");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
