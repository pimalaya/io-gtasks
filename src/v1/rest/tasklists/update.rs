//! Replace a task list (`tasklists.update`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasklists/update>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gtasks_try,
    v1::{
        rest::tasklists::GtasksTaskList,
        send::{GtasksSend, GtasksSendError, GtasksSendOutput, api_url},
    },
};

/// I/O-free coroutine replacing a task list (`tasklists.update`).
pub struct GtasksTaskListUpdate {
    send: GtasksSend<GtasksTaskList>,
}

impl GtasksTaskListUpdate {
    /// Builds the `tasklists.update` request, replacing the task list
    /// of the given id as a whole.
    pub fn new(
        auth: &HttpAuthBearer,
        tasklist_id: &str,
        tasklist: &GtasksTaskList,
    ) -> Result<Self, GtasksSendError> {
        debug!("prepare task list for update");
        trace!("tasklist_id: {tasklist_id:?}");
        trace!("tasklist: {tasklist:?}");

        let url = api_url(["users", "@me", "lists", tasklist_id])?;
        let send = GtasksSend::put_json(auth, url, tasklist)?;

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskListUpdate {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTaskList>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task list updated");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
