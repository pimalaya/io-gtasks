---
cairn: tasks
change: family-audit-fixes
---

- [x] Refuse a path segment the URL would drop, with `GtasksSendError::InvalidRequest` (GTASKS-02)
- [x] Refuse an etag holding a control character (GCAL-09)
- [x] Carry the reasons, the canonical status, the retry delay and the keep-alive flag on `GtasksApiError` (GTASKS-01)
- [x] Read an unknown task status as `GtasksTaskStatus::Unknown` and leave it out of a write (GTASKS-03)
- [x] State what `to_ical` does with `deleted`, `hidden` and an unknown status (GTASKS-04)
- [x] Offline tests, each one seen failing before its fix
- [x] Fold the spec, write the log entry, CHANGELOG.md
