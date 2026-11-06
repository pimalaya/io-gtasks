//! Clear the completed tasks of a task list (`tasks.clear`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks/clear>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};

use crate::{
    coroutine::*,
    gtasks_try,
    v1::send::{GtasksNoResponse, GtasksSend, GtasksSendError, GtasksSendOutput, api_url},
};

/// I/O-free coroutine clearing the completed tasks of a task list
/// (`tasks.clear`).
pub struct GtasksTasksClear {
    send: GtasksSend<GtasksNoResponse>,
}

impl GtasksTasksClear {
    /// Builds the `tasks.clear` request for the given task list id.
    ///
    /// The completed tasks are hidden rather than deleted: a listing
    /// with `showHidden` still returns them.
    pub fn new(auth: &HttpAuthBearer, tasklist_id: &str) -> Result<Self, GtasksSendError> {
        debug!("prepare task list clear");
        trace!("tasklist_id: {tasklist_id:?}");

        let url = api_url(["lists", tasklist_id, "clear"])?;
        let send = GtasksSend::post_empty(auth, url);

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTasksClear {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksNoResponse>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task list cleared");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
