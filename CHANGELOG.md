# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added the I/O-free coroutines for the whole Google Tasks API v1: task lists and tasks.
- Added `GtasksField`, the tri-state of the writable task and task list fields.

  An unset field stays out of a write, a null one clears the stored value.
- Added an optional `If-Match` guard to task update, patch and delete.

  `GtasksSendError::is_precondition_failed` recognises the 412 a stale tag returns.
- Added `GtasksTask::to_ical` and `GtasksTask::from_ical` behind the `ical` feature.

  The VTODO read back is a patch of the fields the projection maps, clearing what the document dropped.
- Added `GtasksClientStd`, a std blocking client behind the `client` feature.

[unreleased]: https://github.com/pimalaya/io-gtasks/compare/root..HEAD
