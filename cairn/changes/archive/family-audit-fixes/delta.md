---
cairn: delta
change: family-audit-fixes
---

## ADDED Requirements

The first two belong to the api capability, the third to ical.

### Requirement: An API error keeps what a caller backs off on
`GtasksSendError::Api` SHALL carry a `GtasksApiError`: the effective status (`error.code`, else the HTTP status), the message, the reasons of `error.errors[]`, the canonical `error.status`, the reasons of `error.details[]`, the retry delay and whether the server allows reusing the connection. The retry delay SHALL be the `Retry-After` header in its seconds form, else the `retryDelay` of the envelope; a `Retry-After` date SHALL read as no delay, since the core has no clock to subtract from. `is_rate_limited` SHALL hold for a 429, for a `RESOURCE_EXHAUSTED` status and for a 403 whose reason is `rateLimitExceeded`, `userRateLimitExceeded`, `quotaExceeded` or `RATE_LIMIT_EXCEEDED`, and SHALL NOT hold for `dailyLimitExceeded`. `is_retryable` SHALL hold for a rate limit and for a 500, 502, 503 or 504.

#### Scenario: A throttled listing
- GIVEN a 403 answer whose envelope lists the reason `rateLimitExceeded`, with the header `Retry-After: 120`
- WHEN the listing completes
- THEN the error is retryable, rate limited, and its retry delay is 120 seconds

### Requirement: An unknown task status reads as unknown
A `status` value the crate does not know SHALL read as `GtasksTaskStatus::Unknown` rather than fail the whole response. A task holding it SHALL leave `status` out of every body it is written in, so a write never states a status the server did not give.

### Requirement: The projection leaves deleted and hidden to the caller
`to_ical` SHALL project neither `deleted` nor `hidden`: a deleted task is written by its status like a live one, and dropping it before projecting is the caller's. A hidden task is a completed one and is written as `COMPLETED`. An unknown status SHALL be written as `NEEDS-ACTION`.

## MODIFIED Requirements

### Requirement: Path segments are encoded one by one
A task list or task id SHALL be percent-encoded as one path segment, so an id holding `/`, `?`, `#` or a space never reads as a separator, a query or a fragment. `@default` and `@me` SHALL reach the wire unchanged. An id the URL would not carry as written SHALL be refused with `GtasksSendError::InvalidRequest` before anything is sent: the empty id, `.`, `..`, and an id holding a tab, a line feed or a carriage return.

#### Scenario: An id with reserved characters
- GIVEN the task list id `a/b?c#d` and the task id `x:y z`
- WHEN the task is fetched
- THEN the request line is `GET /tasks/v1/lists/a%2Fb%3Fc%23d/tasks/x:y%20z`

#### Scenario: A dot id
- GIVEN the task id `..`
- WHEN the request is built
- THEN it is refused as an invalid request, where it used to send the listing of the task list

### Requirement: Task writes take an etag guard
`tasks.update`, `tasks.patch` and `tasks.delete` SHALL take an optional etag sent as `If-Match`. A stale etag comes back as HTTP 412, which `GtasksSendError::is_precondition_failed` SHALL recognise. An etag holding an ASCII control character SHALL be refused with `GtasksSendError::InvalidRequest` before anything is sent, since it would end the header line it rides on. Task list writes SHALL take no guard: the server ignores `If-Match` on them.

## REMOVED Requirements
