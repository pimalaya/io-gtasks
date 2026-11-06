//! List the task lists of the user (`tasklists.list`).
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasklists/list>

use io_http::rfc6750::bearer::HttpAuthBearer;
use log::{debug, trace};
use serde::Serialize;

use crate::{
    coroutine::*,
    gtasks_try,
    v1::{
        query::append_query_pairs,
        rest::tasklists::GtasksTaskLists,
        send::{GtasksSend, GtasksSendError, GtasksSendOutput, api_url},
    },
};

/// Query parameters for listing task lists (`tasklists.list`).
#[derive(Debug, Clone, Default, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksTaskListsListParams<'a> {
    /// The maximum number of task lists to return per page, 1000 by
    /// default and at most.
    pub max_results: Option<u32>,
    /// The page token from a previous list response.
    pub page_token: Option<&'a str>,
}

/// I/O-free coroutine listing the task lists of the user
/// (`tasklists.list`).
pub struct GtasksTaskListsList {
    send: GtasksSend<GtasksTaskLists>,
}

impl GtasksTaskListsList {
    /// Builds the `tasklists.list` request from the given query
    /// parameters.
    pub fn new(
        auth: &HttpAuthBearer,
        params: &GtasksTaskListsListParams,
    ) -> Result<Self, GtasksSendError> {
        debug!("prepare task lists listing");
        trace!("params: {params:?}");

        let mut url = api_url(["users", "@me", "lists"])?;
        append_query_pairs(&mut url, params);

        let send = GtasksSend::get(auth, url);

        Ok(Self { send })
    }
}

impl GtasksCoroutine for GtasksTaskListsList {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<GtasksTaskLists>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        let out = gtasks_try!(&mut self.send, arg);
        debug!("task lists listed");
        trace!("out: {out:?}");
        GtasksCoroutineState::Complete(Ok(out))
    }
}
