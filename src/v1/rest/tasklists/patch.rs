//! Patch a task list (`tasklists.patch`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasklists/patch>

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

/// I/O-free coroutine patching a task list (`tasklists.patch`).
pub struct GtasksTaskListPatch {
    send: GtasksSend<GtasksTaskList>,
}

impl GtasksTaskListPatch {
    /// Builds the `tasklists.patch` request, merging only the fields
    /// the given task list sets.
    pub fn new(
        auth: &HttpAuthBearer,
        tasklist_id: &str,
        tasklist: &GtasksTaskList,
    ) -> Result<Self, GtasksSendError> {
        debug!("prepare task list for patch");
        trace!("tasklist_id: {tasklist_id:?}");
        trace!("tasklist: {tasklist:?}");

        let url = api_url(["users", "@me", "lists", tasklist_id])?;
        let send = GtasksSend::patch_json(auth, url, tasklist)?;

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskListPatch {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTaskList>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task list patched");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
