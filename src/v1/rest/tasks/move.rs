//! Move a task (`tasks.move`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks/move>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;

use crate::{
    coroutine::*,
    gtasks_try,
    v1::{
        query::append_query_pairs,
        rest::tasks::GtasksTask,
        send::{GtasksSend, GtasksSendError, GtasksSendOutput, api_url},
    },
};

/// Query parameters for moving a task (`tasks.move`).
///
/// Leaving every one unset moves the task first among the top-level
/// tasks of its own list.
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksTaskMoveParams<'a> {
    /// The new parent of the task, which must exist in the destination
    /// list and not be hidden. Unset moves the task to the top level.
    /// An assigned or repeating task can neither become a parent nor
    /// get one, and a completed hidden task cannot be nested.
    pub parent: Option<&'a str>,
    /// The new previous sibling of the task, which must exist and not
    /// be hidden. Unset moves the task first among its siblings, the
    /// only place a completed hidden task can go.
    pub previous: Option<&'a str>,
    /// The list to move the task to. Unset moves it within its own
    /// list. A repeating task cannot change lists.
    pub destination_tasklist: Option<&'a str>,
}

/// I/O-free coroutine moving a task (`tasks.move`).
pub struct GtasksTaskMove {
    send: GtasksSend<GtasksTask>,
}

impl GtasksTaskMove {
    /// Builds the `tasks.move` request for the given task list and task
    /// id.
    pub fn new(
        auth: &HttpAuthBearer,
        tasklist_id: &str,
        task_id: &str,
        params: &GtasksTaskMoveParams,
    ) -> Result<Self, GtasksSendError> {
        debug!("prepare task move");
        trace!("tasklist_id: {tasklist_id:?}");
        trace!("task_id: {task_id:?}");
        trace!("params: {params:?}");

        let mut url = api_url(["lists", tasklist_id, "tasks", task_id, "move"])?;
        append_query_pairs(&mut url, params);

        let send = GtasksSend::post_empty(auth, url);

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskMove {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTask>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task moved");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
