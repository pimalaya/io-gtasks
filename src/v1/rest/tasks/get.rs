//! Get a task (`tasks.get`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks/get>

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

/// I/O-free coroutine getting a task by id (`tasks.get`).
pub struct GtasksTaskGet {
    send: GtasksSend<GtasksTask>,
}

impl GtasksTaskGet {
    /// Builds the `tasks.get` request for the given task list and task
    /// id.
    pub fn new(
        auth: &HttpAuthBearer,
        tasklist_id: &str,
        task_id: &str,
    ) -> Result<Self, GtasksSendError> {
        debug!("prepare task retrieval");
        trace!("tasklist_id: {tasklist_id:?}");
        trace!("task_id: {task_id:?}");

        let url = api_url(["lists", tasklist_id, "tasks", task_id])?;
        let send = GtasksSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskGet {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTask>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task retrieved");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
