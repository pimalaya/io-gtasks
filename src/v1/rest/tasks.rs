//! Tasks (`tasks`): list, get, insert, update, patch, delete, move,
//! clear.
//!
//! A task belongs to one task list, either at its top level or under a
//! parent task, ordered among its siblings by a position only the
//! `move` method changes; moving it to another list keeps its id.
//! Deleting a task flags it rather than erasing it, and clearing a list
//! hides its completed tasks. Completing or deleting a parent task does
//! the same to its subtasks.
//!
//! <https://developers.google.com/workspace/tasks/reference/rest/v1/tasks>

use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

use crate::v1::field::GtasksField;

pub mod clear;
pub mod delete;
pub mod get;
#[cfg(feature = "ical")]
pub mod ical;
pub mod insert;
pub mod list;
pub mod r#move;
pub mod patch;
pub mod update;

/// The tasks of a task list, one page of the listing.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksTasks {
    /// Type of the collection, always `tasks#tasks`.
    #[serde(default)]
    pub kind: Option<String>,
    /// ETag of the collection.
    #[serde(default)]
    pub etag: Option<String>,
    /// The tasks of the current page.
    #[serde(default)]
    pub items: Vec<GtasksTask>,
    /// The token retrieving the next page, absent on the last page.
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// A task of a task list.
///
/// The writable fields are [`GtasksField`]s, so a patch body tells a
/// field it leaves alone from one it clears; the read-only ones are
/// plain options the server fills.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksTask {
    /// Type of the resource, always `tasks#task`, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Identifier of the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// ETag of the resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// Title of the task, 1024 characters at most.
    #[serde(default, skip_serializing_if = "GtasksField::is_unset")]
    pub title: GtasksField<String>,
    /// Last modification time of the task, as an RFC 3339 timestamp,
    /// read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    /// Absolute link to the task in the API, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_link: Option<String>,
    /// Identifier of the parent task, absent for a top-level task,
    /// read-only: `tasks.move` changes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Position of the task among its siblings, ordered
    /// lexicographically, read-only: `tasks.move` changes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// Notes describing the task, 8192 characters at most. A task
    /// assigned from a document cannot carry any.
    #[serde(default, skip_serializing_if = "GtasksField::is_unset")]
    pub notes: GtasksField<String>,
    /// Status of the task.
    #[serde(default, skip_serializing_if = "GtasksField::is_unset")]
    pub status: GtasksField<GtasksTaskStatus>,
    /// Scheduled date of the task, as an RFC 3339 timestamp whose time
    /// is always midnight UTC: the API records the date alone and
    /// discards any time a write gives.
    #[serde(default, skip_serializing_if = "GtasksField::is_unset")]
    pub due: GtasksField<String>,
    /// Completion time of the task, as an RFC 3339 timestamp, absent
    /// while the task is not completed.
    #[serde(default, skip_serializing_if = "GtasksField::is_unset")]
    pub completed: GtasksField<String>,
    /// Whether the task is deleted. An assigned task is only deleted
    /// through [`delete`].
    #[serde(default, skip_serializing_if = "GtasksField::is_unset")]
    pub deleted: GtasksField<bool>,
    /// Whether the task is hidden, which a completed task becomes when
    /// its list is cleared, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Links attached to the task, such as the mail it was created
    /// from, read-only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<GtasksTaskLink>,
    /// Absolute link to the task in the Tasks web UI, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_view_link: Option<String>,
    /// Where an assigned task comes from, read-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_info: Option<GtasksAssignmentInfo>,
}

/// The status of a task.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum GtasksTaskStatus {
    /// The task is still to be done.
    NeedsAction,
    /// The task is done.
    Completed,
}

/// A link attached to a task.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksTaskLink {
    /// Type of the link, such as `email`, `generic`, `chat_message` or
    /// `keep_note`.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub link_type: Option<String>,
    /// Description of the link, possibly empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

/// Where a task assigned to the user comes from.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksAssignmentInfo {
    /// Absolute link to the original task where it was assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_to_task: Option<String>,
    /// The kind of surface the task was assigned from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_type: Option<GtasksSurfaceType>,
    /// The document the task was assigned from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive_resource_info: Option<GtasksDriveResourceInfo>,
    /// The chat space the task was assigned from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_info: Option<GtasksSpaceInfo>,
}

/// The kind of surface an assigned task comes from.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GtasksSurfaceType {
    /// A mail.
    Gmail,
    /// A document.
    Document,
    /// A chat space.
    Space,
    /// An unknown surface, which any value this crate does not know
    /// also reads as.
    #[serde(other)]
    ContextTypeUnspecified,
}

/// The document an assigned task comes from.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksDriveResourceInfo {
    /// Identifier of the file in the Drive API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive_file_id: Option<String>,
    /// Resource key needed to open a file shared by link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_key: Option<String>,
}

/// The chat space an assigned task comes from.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GtasksSpaceInfo {
    /// The space, as `spaces/{space}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space: Option<String>,
}
