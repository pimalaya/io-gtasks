//! Create a task (`tasks.insert`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks/insert>

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

/// I/O-free coroutine creating a task (`tasks.insert`).
pub struct GtasksTaskInsert {
    send: GtasksSend<GtasksTask>,
}

impl GtasksTaskInsert {
    /// Builds the `tasks.insert` request from the given task.
    ///
    /// `parent` creates it as a subtask of that task rather than at the
    /// top level, and `previous` right after that sibling rather than
    /// first among its siblings. An assigned task can neither be a
    /// parent nor have one.
    pub fn new(
        auth: &HttpAuthBearer,
        tasklist_id: &str,
        task: &GtasksTask,
        parent: Option<&str>,
        previous: Option<&str>,
    ) -> Result<Self, GtasksSendError> {
        debug!("prepare task for creation");
        trace!("tasklist_id: {tasklist_id:?}");
        trace!("task: {task:?}");
        trace!("parent: {parent:?}");
        trace!("previous: {previous:?}");

        let mut url = api_url(["lists", tasklist_id, "tasks"])?;

        if let Some(parent) = parent {
            url.query_pairs_mut().append_pair("parent", parent);
        }

        if let Some(previous) = previous {
            url.query_pairs_mut().append_pair("previous", previous);
        }

        let send = GtasksSend::post_json(auth, url, task)?;

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskInsert {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTask>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task created");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
