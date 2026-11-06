//! Get a task list (`tasklists.get`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasklists/get>

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

/// I/O-free coroutine getting a task list by id (`tasklists.get`).
pub struct GtasksTaskListGet {
    send: GtasksSend<GtasksTaskList>,
}

impl GtasksTaskListGet {
    /// Builds the `tasklists.get` request for the given task list id,
    /// `@default` naming the default list of the user.
    pub fn new(auth: &HttpAuthBearer, tasklist_id: &str) -> Result<Self, GtasksSendError> {
        debug!("prepare task list retrieval");
        trace!("tasklist_id: {tasklist_id:?}");

        let url = api_url(["users", "@me", "lists", tasklist_id])?;
        let send = GtasksSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskListGet {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTaskList>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task list retrieved");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
