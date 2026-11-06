//! List the tasks of a task list (`tasks.list`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks/list>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;

use crate::{
    coroutine::*,
    gtasks_try,
    v1::{
        query::{append_query_pairs, is_false},
        rest::tasks::GtasksTasks,
        send::{GtasksSend, GtasksSendError, GtasksSendOutput, api_url},
    },
};

/// Query parameters for listing tasks (`tasks.list`).
///
/// The API has no sync token: an incremental listing filters on
/// `updated_min` instead, with `show_deleted` and `show_hidden` set so
/// that deletions and the tasks completed in other clients come back
/// too.
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksTasksListParams<'a> {
    /// The upper bound on the completion time of the returned tasks, as
    /// an RFC 3339 timestamp.
    pub completed_max: Option<&'a str>,
    /// The lower bound on the completion time of the returned tasks, as
    /// an RFC 3339 timestamp.
    pub completed_min: Option<&'a str>,
    /// The upper bound on the due date of the returned tasks, as an RFC
    /// 3339 timestamp.
    pub due_max: Option<&'a str>,
    /// The lower bound on the due date of the returned tasks, as an RFC
    /// 3339 timestamp.
    pub due_min: Option<&'a str>,
    /// The maximum number of tasks to return per page, 20 by default
    /// and 100 at most.
    pub max_results: Option<u32>,
    /// The page token from a previous list response.
    pub page_token: Option<&'a str>,
    /// Whether to include the tasks assigned to the user from another
    /// surface, such as a document or a chat space.
    #[serde(skip_serializing_if = "is_false")]
    pub show_assigned: bool,
    /// Whether to include the completed tasks, true by default. The
    /// tasks completed in another client are hidden as well, so they
    /// also need `show_hidden`.
    pub show_completed: Option<bool>,
    /// Whether to include the deleted tasks.
    #[serde(skip_serializing_if = "is_false")]
    pub show_deleted: bool,
    /// Whether to include the hidden tasks: the ones cleared from their
    /// list, and the ones completed in another client.
    #[serde(skip_serializing_if = "is_false")]
    pub show_hidden: bool,
    /// The lower bound on the last modification time of the returned
    /// tasks, as an RFC 3339 timestamp.
    pub updated_min: Option<&'a str>,
}

/// I/O-free coroutine listing the tasks of a task list (`tasks.list`).
pub struct GtasksTasksList {
    send: GtasksSend<GtasksTasks>,
}

impl GtasksTasksList {
    /// Builds the `tasks.list` request from the given query parameters.
    pub fn new(
        auth: &HttpAuthBearer,
        tasklist_id: &str,
        params: &GtasksTasksListParams,
    ) -> Result<Self, GtasksSendError> {
        debug!("prepare tasks listing");
        trace!("tasklist_id: {tasklist_id:?}");
        trace!("params: {params:?}");

        let mut url = api_url(["lists", tasklist_id, "tasks"])?;
        append_query_pairs(&mut url, params);

        let send = GtasksSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTasksList {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTasks>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("tasks listed");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
