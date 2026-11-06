---
cairn: spec
capability: ical
status: current
---

# The VTODO projection

Behind the `ical` feature, `GtasksTask::to_ical` writes a task as an iCalendar document holding one VTODO (RFC 5545 3.6.2) and `GtasksTask::from_ical` reads a VTODO back as a task. A task holds little, so most of what a VTODO can carry has nowhere to go:

| Task          | VTODO                                       | Direction        |
|---------------|---------------------------------------------|------------------|
| `id`          | `UID`                                       | written only     |
| `updated`     | `DTSTAMP`, `LAST-MODIFIED`                  | written only     |
| `title`       | `SUMMARY`                                   | both             |
| `notes`       | `DESCRIPTION`                               | both             |
| `status`      | `STATUS`                                    | both             |
| `completed`   | `COMPLETED`                                 | both             |
| `due`         | `DUE;VALUE=DATE`                            | both             |
| `parent`      | `RELATED-TO;RELTYPE=PARENT`                 | both, read-only on the wire |
| `webViewLink` | `X-GOOGLE-WEB-VIEW-LINK`                    | written only     |
| `links`       | `LINK;VALUE=URI;LINKREL=related;LABEL=...` (RFC 9253 6.1) | written only |

### Requirement: The way back is a patch of the mapped fields
`from_ical` SHALL read the first VTODO of the document and return a task whose mapped writable fields (`title`, `notes`, `status`, `due`, `completed`) are set from the VTODO, or null when it lacks them, every other writable field unset. Sent as a patch, it SHALL replace what the projection manages, clear what the document dropped and leave the rest alone. Sent as an insert, the nulls are accepted.

#### Scenario: A VTODO from a richer client
- GIVEN a VTODO with a SUMMARY, a NEEDS-ACTION STATUS, CATEGORIES, a PRIORITY, an `X-` property and a VALARM, and no DESCRIPTION or DUE
- WHEN it is read back
- THEN the task serializes as `{"title":...,"notes":null,"status":"needsAction","due":null,"completed":null}` and the rest is dropped

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
