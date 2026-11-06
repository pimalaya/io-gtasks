//! Task lists (`tasklists`): list, get, insert, update, patch, delete.
//!
//! A task list is the container of tasks, addressed by its id, or by
//! the alias `@default` for the authenticated user's default list. Its
//! title is the only field a client writes.
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasklists>

use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

use crate::v1::field::GtasksField;

pub mod delete;
pub mod get;
pub mod insert;
pub mod list;
pub mod patch;
pub mod update;

/// The task lists of the user, one page of the listing.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksTaskLists {
    /// Type of the collection, always `tasks#taskLists`.
    #[serde(default)]
    pub kind: Option<String>,
    /// ETag of the collection.
    #[serde(default)]
    pub etag: Option<String>,
    /// The task lists of the current page.
    #[serde(default)]
    pub items: Vec<GtasksTaskList>,
    /// The token retrieving the next page, absent on the last page.
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// A list of tasks.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksTaskList {
    /// Type of the resource, always `tasks#taskList`, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Identifier of the task list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// ETag of the resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Title of the task list, 1024 characters at most.
    #[serde(default, skip_serializing_if = "GtasksField::is_unset")]
    pub title: GtasksField<String>,
    /// Last modification time of the task list, as an RFC 3339
    /// timestamp, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    /// Absolute link to the task list in the API, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_link: Option<String>,
}
