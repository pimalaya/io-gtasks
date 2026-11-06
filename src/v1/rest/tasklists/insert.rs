//! Create a task list (`tasklists.insert`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasklists/insert>

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

/// I/O-free coroutine creating a task list (`tasklists.insert`).
pub struct GtasksTaskListInsert {
    send: GtasksSend<GtasksTaskList>,
}

impl GtasksTaskListInsert {
    /// Builds the `tasklists.insert` request from the given task list.
    pub fn new(auth: &HttpAuthBearer, tasklist: &GtasksTaskList) -> Result<Self, GtasksSendError> {
        debug!("prepare task list for creation");
        trace!("tasklist: {tasklist:?}");

        let url = api_url(["users", "@me", "lists"])?;
        let send = GtasksSend::post_json(auth, url, tasklist)?;

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskListInsert {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTaskList>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task list created");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
