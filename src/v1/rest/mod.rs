//! Google Tasks REST API, mirroring the reference tree: each resource
//! is a module and each method a file named after the API method in
//! snake_case.
//!
//! <https://developers.google.com/workspace/tasks/reference/rest>

pub mod tasklists;
pub mod tasks;
