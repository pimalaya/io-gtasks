# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **BREAKING**: Changed the pimalaya-stream dependency to 0.4.

  Its `tls::Tls`, the `tls` field of `GtasksClientStdConnectOptions`, moved the trusted certificate from `cert` to `server`, and gained `client`, the certificate presented when the server asks for one (mutual TLS).

- **BREAKING**: Changed the io-http dependency to 0.8.

  Its `HttpAuthBearer` is the credential every coroutine and `GtasksClientStd` take.

## [0.1.0] - 2026-11-13

### Added

- Added the I/O-free coroutines for the whole Google Tasks API v1: task lists and tasks.

  They build on io-http 0.7, whose `HttpAuthBearer` carries the access token.
- Added `GtasksField`, the tri-state of the writable task and task list fields.

  An unset field stays out of a write, a null one clears the stored value.
- Added an optional `If-Match` guard to task update, patch and delete.

  `GtasksSendError::is_precondition_failed` recognises the 412 a stale tag returns.
- Added `GtasksApiError`, the error envelope of a non-2xx answer.

  It keeps the reasons, the canonical status, the delay the server asked to wait and whether the connection can be reused.
- Added `GtasksSendError::InvalidRequest`, the refusal of a request before it is sent.

  It covers an id the path would drop (empty, `.`, `..`, or holding a tab or a line break), an etag holding a control character, and a method or a header that would add a line to the request, such as a token holding a line break.
- Added `GtasksTaskStatus::Unknown`, the fallback of a status the crate does not know.

  It reads without failing the response and stays out of every write.
- Added `GtasksTask::to_ical` and `GtasksTask::from_ical` behind the `ical` feature.

  The VTODO read back is a patch of the fields the projection maps, clearing what the document dropped.
- Added `GtasksClientStd`, a std blocking client behind the `client` feature.

[unreleased]: https://github.com/pimalaya/io-gtasks/compare/v0.1.0..HEAD
[0.1.0]: https://github.com/pimalaya/io-gtasks/compare/root..v0.1.0
