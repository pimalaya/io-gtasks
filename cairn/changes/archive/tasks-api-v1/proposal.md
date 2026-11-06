---
cairn: change
id: tasks-api-v1
status: landed
created: 2026-11-06
---

# Mirror the Google Tasks API v1

## Why

A Google account keeps its tasks in the Tasks API only: its CalDAV server refuses a VTODO (403, recorded by unfog's gtd-hardening on 2026-10-29), so neither the Android app nor unfog can read or write a Google task today. The Android app asked for the crate on 2026-11-06 to back task lists as collections of their own, and unfog's google-tasks change lists it as its first step.

## What

- A new io-gtasks crate, sibling of io-gcal and io-gpeople: I/O-free coroutines over io-http for the two resources of the API (task lists and tasks, 14 methods), a blocking std client behind `client`, the TLS features of the family.
- The writable fields of a resource are tri-state (unset, null, set), so a patch can clear a field without touching the others. Task writes take an optional etag guard, the one place the API honours `If-Match`.
- An `ical` feature projecting a task onto a VTODO and back, the way back being a patch of the mapped fields only, so a write from a richer client never clears what the task cannot hold.
- Live tests on the pimalaya.org service account and the delegated test user, on throwaway lists only, and the incremental sync recipe the API leaves to its callers.

## Out

- Where GTD states, categories and priority live on a Google account: the app and unfog own that decision (unfog google-tasks open question).
- Publishing: the crate stays unpublished, pinned by git revision, until the owner releases it.
