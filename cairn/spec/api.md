---
cairn: spec
capability: api
status: current
---

# The Tasks API surface

The crate mirrors version 1 of the Tasks REST API: two resources, task lists and tasks, fourteen methods. src/lib.rs describes the code layout; this file states the contract each coroutine keeps.

### Requirement: One method, one exchange
Each method SHALL be one coroutine sending one request to `https://tasks.googleapis.com/tasks/v1/`, authorized by a bearer token, and completing with the parsed 2xx JSON body, the error envelope as `GtasksSendError::Api`, or `UnexpectedRedirect` for a 3xx, which is never followed. An empty 2xx body SHALL parse into `GtasksNoResponse`.

### Requirement: Path segments are encoded one by one
A task list or task id SHALL be percent-encoded as one path segment, so an id holding `/`, `?`, `#` or a space never reads as a separator, a query or a fragment. `@default` and `@me` SHALL reach the wire unchanged.

#### Scenario: An id with reserved characters
- GIVEN the task list id `a/b?c#d` and the task id `x:y z`
- WHEN the task is fetched
- THEN the request line is `GET /tasks/v1/lists/a%2Fb%3Fc%23d/tasks/x:y%20z`

### Requirement: Writable fields are tri-state
The writable fields of a task (`title`, `notes`, `status`, `due`, `completed`, `deleted`) and of a task list (`title`) SHALL be `GtasksField`s: an unset field SHALL stay out of the body, a null one SHALL be sent as JSON `null`, which clears the stored value, and a set one as its value. A read SHALL map a missing field to unset and a JSON `null` to null. Read-only fields SHALL be options, sent when set; the server ignores them in a write.

#### Scenario: A patch clearing the notes and the due date
- GIVEN a task whose `notes` and `due` are null and every other field unset
- WHEN it is sent as a patch
- THEN the body is `{"notes":null,"due":null}` and the server drops both while keeping the title

### Requirement: Task writes take an etag guard
`tasks.update`, `tasks.patch` and `tasks.delete` SHALL take an optional etag sent as `If-Match`. A stale etag comes back as HTTP 412, which `GtasksSendError::is_precondition_failed` SHALL recognise. Task list writes SHALL take no guard: the server ignores `If-Match` on them.

### Requirement: List flags stay off the wire at their default
`showDeleted`, `showHidden` and `showAssigned` SHALL only be sent when true. `showCompleted`, true by default on the server, SHALL only be sent when the caller sets it, so `Some(false)` is the only way to drop completed tasks. A listing with no parameter SHALL carry no query at all.

### Requirement: An unknown assignment surface reads as unspecified
A `surfaceType` value the crate does not know SHALL read as `GtasksSurfaceType::ContextTypeUnspecified` rather than fail the whole response.

### Requirement: Inserting and moving take the position
`tasks.insert` SHALL take the parent and the previous sibling positionally and `tasks.move` SHALL take them, plus the destination list, in `GtasksTaskMoveParams`; each one left unset SHALL stay off the query. The `parent` field of a task body SHALL NOT be relied on to nest it: the server ignores it on every write.
