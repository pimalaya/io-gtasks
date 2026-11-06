---
cairn: delta
change: tasks-api-v1
---

## ADDED Requirements

### Requirement: One method, one exchange
Each method SHALL be one coroutine sending one request to `https://tasks.googleapis.com/tasks/v1/`, authorized by a bearer token, and completing with the parsed 2xx JSON body, the error envelope as `GtasksSendError::Api`, or `UnexpectedRedirect` for a 3xx, which is never followed. An empty 2xx body SHALL parse into `GtasksNoResponse`.

### Requirement: Path segments are encoded one by one
A task list or task id SHALL be percent-encoded as one path segment, so an id holding `/`, `?`, `#` or a space never reads as a separator, a query or a fragment. `@default` and `@me` SHALL reach the wire unchanged.

### Requirement: Writable fields are tri-state
The writable fields of a task (`title`, `notes`, `status`, `due`, `completed`, `deleted`) and of a task list (`title`) SHALL be `GtasksField`s: an unset field SHALL stay out of the body, a null one SHALL be sent as JSON `null`, which clears the stored value, and a set one as its value. A read SHALL map a missing field to unset and a JSON `null` to null. Read-only fields SHALL be options, sent when set; the server ignores them in a write.

### Requirement: Task writes take an etag guard
`tasks.update`, `tasks.patch` and `tasks.delete` SHALL take an optional etag sent as `If-Match`. A stale etag comes back as HTTP 412, which `GtasksSendError::is_precondition_failed` SHALL recognise. Task list writes SHALL take no guard: the server ignores `If-Match` on them.

### Requirement: List flags stay off the wire at their default
`showDeleted`, `showHidden` and `showAssigned` SHALL only be sent when true. `showCompleted`, true by default on the server, SHALL only be sent when the caller sets it, so `Some(false)` is the only way to drop completed tasks. A listing with no parameter SHALL carry no query at all.

### Requirement: An unknown assignment surface reads as unspecified
A `surfaceType` value the crate does not know SHALL read as `GtasksSurfaceType::ContextTypeUnspecified` rather than fail the whole response.

### Requirement: Inserting and moving take the position
`tasks.insert` SHALL take the parent and the previous sibling positionally and `tasks.move` SHALL take them, plus the destination list, in `GtasksTaskMoveParams`; each one left unset SHALL stay off the query. The `parent` field of a task body SHALL NOT be relied on to nest it: the server ignores it on every write.
### Requirement: The way back is a patch of the mapped fields
`from_ical` SHALL read the first VTODO of the document and return a task whose mapped writable fields (`title`, `notes`, `status`, `due`, `completed`) are set from the VTODO, or null when it lacks them, every other writable field unset. Sent as a patch, it SHALL replace what the projection manages, clear what the document dropped and leave the rest alone. Sent as an insert, the nulls are accepted.

### Requirement: The status follows STATUS, then COMPLETED
A `STATUS:COMPLETED` SHALL read as completed and any other STATUS as needs action. Without a STATUS, a COMPLETED property SHALL read as completed. A completed task SHALL carry the COMPLETED time when it is a UTC date-time, and leave `completed` unset otherwise, so the server keeps or stamps its own. A task that needs action SHALL carry a null `completed`.

### Requirement: A due date keeps the day it is written with
`DUE` SHALL read as the midnight UTC timestamp of the date its value starts with, whatever its value type or zone, since the API stores a date alone and takes the UTC day of any timestamp it is given. `to_ical` SHALL write the UTC day of `due` as a `VALUE=DATE`.

### Requirement: An unreadable value never clears
A `DUE` or `COMPLETED` that does not read as a date SHALL leave its field unset rather than null, so a malformed document cannot clear what the server holds.

### Requirement: The parent comes from RELATED-TO
A `RELATED-TO` with no RELTYPE or `RELTYPE=PARENT` SHALL set `parent` to its value; other relations SHALL be ignored. The caller nests a task through `tasks.insert` or `tasks.move`, since the server ignores `parent` in a write.

### Requirement: The UID is the caller's key
`from_ical` SHALL NOT read the UID into `id`: the caller addresses the task by the id it already holds, and the server assigns its own id on insert, ignoring one in the body.

### Requirement: A document without a VTODO is refused
`from_ical` SHALL refuse a document that does not parse (`Parse`), one whose components hold no VTODO (`NotATodo`, naming the first component other than a VTIMEZONE) and one with no component at all (`NoTodo`).
The sync capability is prose only: the server behaviour the incremental listing relies on, folded as cairn/spec/sync.md.

## MODIFIED Requirements

## REMOVED Requirements
