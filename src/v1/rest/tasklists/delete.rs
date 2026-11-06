//! Delete a task list (`tasklists.delete`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasklists/delete>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gtasks_try,
    v1::send::{GtasksNoResponse, GtasksSend, GtasksSendError, GtasksSendOutput, api_url},
};

/// I/O-free coroutine deleting a task list (`tasklists.delete`).
pub struct GtasksTaskListDelete {
    send: GtasksSend<GtasksNoResponse>,
}

impl GtasksTaskListDelete {
    /// Builds the `tasklists.delete` request for the given task list id.
    ///
    /// The default list of the user cannot be deleted.
    pub fn new(auth: &HttpAuthBearer, tasklist_id: &str) -> Result<Self, GtasksSendError> {
        debug!("prepare task list deletion");
        trace!("tasklist_id: {tasklist_id:?}");

        let url = api_url(["users", "@me", "lists", tasklist_id])?;
        let send = GtasksSend::delete(auth, url);

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskListDelete {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksNoResponse>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task list deleted");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
