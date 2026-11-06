---
cairn: spec
capability: sync
status: current
---

# Keeping a local copy in step

The Tasks API has neither a sync token nor a push channel, so a consumer keeping a local copy polls, and the crate ships no watcher. This note records how the server behaves under that loop, as observed live (evidence in the log), since the reference states little of it.

## The incremental listing

A consumer lists each task list with `updatedMin` set to its watermark, `showDeleted` and `showHidden` set, and `showCompleted` left at its default, walking every page. The watermark is the greatest `updated` the previous listing returned, a server clock, never the local one. Without `showHidden`, a task completed in another client stays invisible, since such a task is hidden as well as completed.

A task list's own `updated` moves when a task is added to it but not when one is patched, so it cannot tell a consumer which lists to skip: every list is listed on every poll.

## Deletions

Deleting a task flags it: it keeps its id, `deleted` turns true, its `updated` moves, and both the incremental listing and `tasks.get` still return it. A default listing hides it. Patching `deleted` back to false restores it. A task deleted on its own loses its `parent`.

How long a deleted task stays listable is undocumented and was not observed past a few minutes. A consumer that may miss a deletion (a long offline gap) needs a periodic full listing with `showDeleted` and `showHidden`, treating a task it holds that is absent from the full listing as gone.

## Server-side cascades

A consumer that writes one task sees others change:

- completing a parent completes its subtasks, each stamped with its own `completed`;
- deleting a parent deletes its subtasks, which keep their `parent`;
- reopening a task (status back to needs action) drops its `completed`, whether or not the write cleared it;
- completing a task without a `completed` time stamps the current one;
- clearing a list hides its completed tasks (`hidden` true) and moves their position.

The incremental listing returns every task a cascade touched, since each one's `updated` moves.

## Moves

`tasks.move` with a destination list keeps the task's id, but leaves nothing behind in the source list: no flag, no entry. The task only shows up in the destination's incremental listing. A consumer therefore keys tasks by id across all the lists of an account, so a task appearing in another list reads as a move rather than a deletion plus a creation.

## Dates

`due` holds a date: a write giving a timestamp keeps its UTC day at midnight UTC, and a bare date (`2026-11-10`) is refused with HTTP 400. `completed` keeps the timestamp a write gives.

## Concurrency

Task writes honour `If-Match` and answer HTTP 412 to a stale etag; task list writes ignore it. A consumer guards a task write with the etag of its last read and, on a 412, re-reads and merges before writing again.
