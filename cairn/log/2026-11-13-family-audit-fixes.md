---
cairn: log
change: family-audit-fixes
landed: 2026-11-13
---

# The family audit fixes

Five findings of the family audit of 2026-11-13, each one reproduced offline before its fix.

**An id could vanish from the path (GTASKS-02).** `api_url` handed its segments to the URL builder, which skips `.` and `..` and strips tabs and line breaks from what it is given. Seen on the unfixed code: the task id `..` or `.` sent `GET /tasks/v1/lists/list-1/tasks`, the listing, whose body reads as a task with every field unset, so the call answered `Ok`; `.`, a tab, `.` sent `GET /tasks/v1/lists/list-1/`, one segment higher; `a`, a tab, `b` reached the server as the id `ab`; the empty id and a lone line feed sent a trailing slash. The audit named the dot and empty ids; the tab and line break cases were found while writing the test. `api_url` now refuses such a segment with the new `GtasksSendError::InvalidRequest`. A percent-encoded dot segment (`%2e%2e`) was already safe, its `%` being encoded, and a test now holds that.

**An etag reached the header line as given (GCAL-09).** `"a"`, CR LF, `X-Injected: 1` as `if_match` wrote a header line of its own into the authorized request. `GtasksSend::with_guard` now refuses a guard holding an ASCII control character, which made it and `delete_if_match` fallible. The method, the content type and the token of `with_method` are not checked here: no coroutine passes a caller string there, and the header writer of the HTTP layer is where every caller would be covered.

**The error kept the code and the message only (GTASKS-01).** `GtasksSendError::Api { status, message }` became `Api(GtasksApiError)`, with the reasons of `errors[]`, the canonical status, the reasons of `details[]`, the retry delay and the keep-alive flag, on the shape io-gmail and io-gpeople expose for the first three. The delay is the `Retry-After` header in seconds, else the `retryDelay` of a `google.rpc.RetryInfo` detail; a `Retry-After` date reads as none, the core having no clock. `is_retryable` now holds for a 403 whose reason is a rate limit, which it read as a permanent refusal. Which reasons the Tasks API sends on a quota was not checked live: the list is the one of its sibling APIs. The one consumer binds to `status()`, which is unchanged.

**An unknown status failed the page (GTASKS-03).** A listing holding `"status":"inProgress"` failed with `unknown variant`. It now reads as `GtasksTaskStatus::Unknown`, which no body carries: a full update of a task read that way leaves `status` out rather than send `"unknown"` or overwrite it with `needsAction`.

**The projection said nothing of deleted and hidden (GTASKS-04).** `to_ical` writes a deleted task by its status, like a live one. That stays, since the one consumer drops deleted tasks before projecting and an `Option<String>` would break it for nothing; the method and the spec now say so. The audit's other half does not hold: a hidden task is a completed one, so it was never written as open.

Breaks for a consumer pinned by revision: the shape of `GtasksSendError::Api`, the `Result` of `GtasksSend::with_guard` and `GtasksSend::delete_if_match`, and one more variant on `GtasksSendError` and on `GtasksTaskStatus`.

Capabilities moved: api (MODIFIED "Path segments are encoded one by one" and "Task writes take an etag guard", ADDED "An API error keeps what a caller backs off on" and "An unknown task status reads as unknown"), ical (ADDED "The projection leaves deleted and hidden to the caller").
