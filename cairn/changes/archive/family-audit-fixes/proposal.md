---
cairn: change
id: family-audit-fixes
status: landed
created: 2026-11-13
---

# The family audit fixes

## Why

The family audit of 2026-11-13 found five defects in the crate, each one small, all of them on the edge between the caller and the wire:

- An id could vanish from the path (GTASKS-02). The URL builder skips `.` and `..` and strips tabs and line breaks, so `tasks.get` on the id `..` sent the listing of the list and answered `Ok` with an empty task, `.` followed by a tab and `.` climbed one segment higher, and `a`, a tab, `b` reached the server as the id `ab`. An empty id became a trailing slash.
- An etag was written into `If-Match` as given (GCAL-09, the half of it that lives here), so one holding a CR LF added header lines of its own to an authorized request.
- The error of a non-2xx answer kept the code and the message only (GTASKS-01). A 403 carrying `rateLimitExceeded` read as a permanent refusal, the delay the server asked for was dropped with the response headers, and a core that cannot sleep gave its caller nothing to back off with.
- A `status` outside `needsAction` and `completed` failed the whole page it was listed on (GTASKS-03), where `surfaceType` already had a fallback.
- `to_ical` said nothing of `deleted` and `hidden` (GTASKS-04), which the incremental listing returns.

## What

- `GtasksSendError::InvalidRequest`, returned before anything is sent for a path segment that is empty, `.`, `..` or holds a tab, a line feed or a carriage return, and for an etag holding an ASCII control character.
- `GtasksApiError`, the payload of `GtasksSendError::Api`: the effective status, the message, the reasons of `errors[]`, the canonical `status`, the reasons of `details[]`, the retry delay and whether the connection can be reused. It follows the shape io-gmail and io-gpeople already expose, plus the delay and the keep-alive flag.
- `GtasksTaskStatus::Unknown`, the fallback of a status the crate does not know, left out of every written body.
- The contract of `to_ical` on `deleted`, `hidden` and an unknown status, stated on the method and in the spec.

## Breaks

- `GtasksSendError::Api { status, message }` becomes `GtasksSendError::Api(GtasksApiError)`. `status()`, `is_retryable()` and `is_precondition_failed()` keep their signature.
- `GtasksSend::with_guard` and `GtasksSend::delete_if_match` return a `Result`.
- `GtasksSendError` and `GtasksTaskStatus` each gain a variant.

## Out

- Refusing a control character in the method, the content type or the token of `GtasksSend::with_method`: no coroutine passes a caller string there, and the header writer is the place to refuse it for every caller.
- Capping the fallback message of a non-JSON error body.
- Returning `Option<String>` from `to_ical`: the one consumer already drops deleted tasks first, so the break would buy nothing.
