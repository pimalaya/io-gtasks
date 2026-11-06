//! Offline coverage of the tasks (`tasks`): the eight methods, their
//! query parameters and the task representation.

mod common;

use common::*;
use io_gtasks::v1::{
    field::GtasksField,
    rest::tasks::{
        GtasksSurfaceType, GtasksTask, GtasksTaskStatus,
        clear::GtasksTasksClear,
        delete::GtasksTaskDelete,
        get::GtasksTaskGet,
        insert::GtasksTaskInsert,
        list::{GtasksTasksList, GtasksTasksListParams},
        r#move::{GtasksTaskMove, GtasksTaskMoveParams},
        patch::GtasksTaskPatch,
        update::GtasksTaskUpdate,
    },
};

const TASK: &str = r#"{
  "kind": "tasks#task",
  "id": "task-1",
  "etag": "\"tag-1\"",
  "title": "Buy milk",
  "updated": "2026-11-06T07:52:50.536Z",
  "selfLink": "https://www.googleapis.com/tasks/v1/lists/list-1/tasks/task-1",
  "parent": "task-0",
  "position": "00000000000000000001",
  "notes": "Two litres",
  "status": "completed",
  "due": "2026-11-12T00:00:00.000Z",
  "completed": "2026-11-06T07:52:55.094Z",
  "deleted": true,
  "hidden": true,
  "links": [{"type": "email", "description": "Groceries", "link": "https://mail.google.com/mail/#all/abc"}],
  "webViewLink": "https://tasks.google.com/task/abc?sa=6",
  "assignmentInfo": {
    "linkToTask": "https://docs.google.com/document/d/doc-1",
    "surfaceType": "SOME_FUTURE_SURFACE",
    "driveResourceInfo": {"driveFileId": "doc-1", "resourceKey": "key-1"},
    "spaceInfo": {"space": "spaces/space-1"}
  }
}"#;

fn titled(title: &str) -> GtasksTask {
    GtasksTask {
        title: GtasksField::Set(String::from(title)),
        ..Default::default()
    }
}

#[test]
fn lists_tasks_with_every_filter() {
    let params = GtasksTasksListParams {
        completed_max: Some("2026-11-30T00:00:00Z"),
        completed_min: Some("2026-11-01T00:00:00Z"),
        due_max: Some("2026-12-31T00:00:00Z"),
        due_min: Some("2026-12-01T00:00:00Z"),
        max_results: Some(100),
        page_token: Some("page-2"),
        show_assigned: true,
        show_completed: Some(false),
        show_deleted: true,
        show_hidden: true,
        updated_min: Some("2026-11-06T07:52:00.000Z"),
    };
    let body = format!(r#"{{"kind":"tasks#tasks","items":[{TASK}],"nextPageToken":"page-3"}}"#);
    let mut coroutine = GtasksTasksList::new(&auth(), "list-1", &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", &body));
    let out = ret.unwrap();

    assert_eq!(out.response.items.len(), 1);
    assert_eq!(out.response.next_page_token.as_deref(), Some("page-3"));

    assert!(request_line(&request).starts_with("GET /tasks/v1/lists/list-1/tasks?"));

    for pair in [
        "completedMax=2026-11-30T00%3A00%3A00Z",
        "completedMin=2026-11-01T00%3A00%3A00Z",
        "dueMax=2026-12-31T00%3A00%3A00Z",
        "dueMin=2026-12-01T00%3A00%3A00Z",
        "maxResults=100",
        "pageToken=page-2",
        "showAssigned=true",
        "showCompleted=false",
        "showDeleted=true",
        "showHidden=true",
        "updatedMin=2026-11-06T07%3A52%3A00.000Z",
    ] {
        assert_query(&request, pair);
    }
}

#[test]
fn lists_tasks_without_a_query() {
    let mut coroutine = GtasksTasksList::new(&auth(), "@default", &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    assert!(ret.unwrap().response.items.is_empty());

    // NOTE: the false flags stay off the wire, and so does an unset
    // showCompleted, whose server default is true.
    assert_eq!(request_line(&request), "GET /tasks/v1/lists/@default/tasks");
}

#[test]
fn gets_a_task_with_every_field() {
    let mut coroutine = GtasksTaskGet::new(&auth(), "list-1", "task-1").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", TASK));
    let task = ret.unwrap().response;

    assert_eq!(
        request_line(&request),
        "GET /tasks/v1/lists/list-1/tasks/task-1"
    );

    assert_eq!(task.id.as_deref(), Some("task-1"));
    assert_eq!(task.etag.as_deref(), Some("\"tag-1\""));
    assert_eq!(task.title.as_deref(), Some("Buy milk"));
    assert_eq!(task.parent.as_deref(), Some("task-0"));
    assert_eq!(task.position.as_deref(), Some("00000000000000000001"));
    assert_eq!(task.notes.as_deref(), Some("Two litres"));
    assert_eq!(task.status, GtasksField::Set(GtasksTaskStatus::Completed));
    assert_eq!(task.due.as_deref(), Some("2026-11-12T00:00:00.000Z"));
    assert_eq!(task.completed.as_deref(), Some("2026-11-06T07:52:55.094Z"));
    assert_eq!(task.deleted, GtasksField::Set(true));
    assert_eq!(task.hidden, Some(true));
    assert_eq!(task.links[0].link_type.as_deref(), Some("email"));
    assert_eq!(task.links[0].description.as_deref(), Some("Groceries"));
    assert_eq!(
        task.web_view_link.as_deref(),
        Some("https://tasks.google.com/task/abc?sa=6")
    );

    let assignment = task.assignment_info.expect("the assignment is parsed");
    assert_eq!(
        assignment.surface_type,
        Some(GtasksSurfaceType::ContextTypeUnspecified),
        "an unknown surface reads as unspecified"
    );
    assert_eq!(
        assignment
            .drive_resource_info
            .and_then(|info| info.drive_file_id),
        Some(String::from("doc-1"))
    );
    assert_eq!(
        assignment.space_info.and_then(|info| info.space),
        Some(String::from("spaces/space-1"))
    );
}

#[test]
fn reads_a_minimal_task_as_unset_fields() {
    let mut coroutine = GtasksTaskGet::new(&auth(), "list-1", "task-1").unwrap();
    let body = r#"{"id":"task-1","title":"","status":"needsAction","links":[]}"#;
    let (_, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", body));
    let task = ret.unwrap().response;

    assert_eq!(task.title, GtasksField::Set(String::new()));
    assert_eq!(task.status, GtasksField::Set(GtasksTaskStatus::NeedsAction));
    assert_eq!(task.notes, GtasksField::Unset);
    assert_eq!(task.due, GtasksField::Unset);
    assert_eq!(task.completed, GtasksField::Unset);
    assert_eq!(task.deleted, GtasksField::Unset);
    assert_eq!(task.parent, None);
}

#[test]
fn inserts_a_subtask_after_a_sibling() {
    let task = GtasksTask {
        notes: GtasksField::Null,
        due: GtasksField::Set(String::from("2026-11-10T00:00:00.000Z")),
        ..titled("Buy milk")
    };
    let mut coroutine =
        GtasksTaskInsert::new(&auth(), "list-1", &task, Some("task-0"), Some("task-9")).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", TASK));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "POST /tasks/v1/lists/list-1/tasks?parent=task-0&previous=task-9"
    );
    assert_eq!(
        request_body(&request),
        r#"{"title":"Buy milk","notes":null,"due":"2026-11-10T00:00:00.000Z"}"#
    );
}

#[test]
fn inserts_a_top_level_task() {
    let mut coroutine = GtasksTaskInsert::new(&auth(), "list-1", &titled("a"), None, None).unwrap();
    let (request, _) = expect_exchange(&mut coroutine, &json_response("200 OK", TASK));

    assert_eq!(request_line(&request), "POST /tasks/v1/lists/list-1/tasks");
}

#[test]
fn updates_a_task_under_a_guard() {
    let task = GtasksTask {
        id: Some(String::from("task-1")),
        status: GtasksField::Set(GtasksTaskStatus::NeedsAction),
        ..titled("Buy milk")
    };
    let mut coroutine =
        GtasksTaskUpdate::new(&auth(), "list-1", "task-1", &task, Some("\"tag-1\"")).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", TASK));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "PUT /tasks/v1/lists/list-1/tasks/task-1"
    );
    assert!(request.contains("If-Match: \"tag-1\""), "got: {request}");
    assert_eq!(
        request_body(&request),
        r#"{"id":"task-1","title":"Buy milk","status":"needsAction"}"#
    );
}

#[test]
fn patches_a_task_clearing_what_is_null() {
    let task = GtasksTask {
        notes: GtasksField::Null,
        due: GtasksField::Null,
        completed: GtasksField::Null,
        status: GtasksField::Set(GtasksTaskStatus::NeedsAction),
        ..Default::default()
    };
    let mut coroutine = GtasksTaskPatch::new(&auth(), "list-1", "task-1", &task, None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", TASK));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "PATCH /tasks/v1/lists/list-1/tasks/task-1"
    );
    assert!(!request.contains("If-Match"), "got: {request}");
    assert_eq!(
        request_body(&request),
        r#"{"notes":null,"status":"needsAction","due":null,"completed":null}"#
    );
}

#[test]
fn deletes_a_task() {
    let mut coroutine = GtasksTaskDelete::new(&auth(), "list-1", "task-1", None).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "DELETE /tasks/v1/lists/list-1/tasks/task-1"
    );
}

#[test]
fn moves_a_task_to_another_list() {
    let params = GtasksTaskMoveParams {
        parent: Some("task-0"),
        previous: Some("task-9"),
        destination_tasklist: Some("list-2"),
    };
    let mut coroutine = GtasksTaskMove::new(&auth(), "list-1", "task-1", &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", TASK));

    assert_eq!(ret.unwrap().response.id.as_deref(), Some("task-1"));

    assert_eq!(
        request_line(&request),
        "POST /tasks/v1/lists/list-1/tasks/task-1/move?parent=task-0&previous=task-9&destinationTasklist=list-2"
    );
    assert_eq!(request_body(&request), "");
}

#[test]
fn moves_a_task_first_at_the_top_level() {
    let mut coroutine =
        GtasksTaskMove::new(&auth(), "list-1", "task-1", &Default::default()).unwrap();
    let (request, _) = expect_exchange(&mut coroutine, &json_response("200 OK", TASK));

    assert_eq!(
        request_line(&request),
        "POST /tasks/v1/lists/list-1/tasks/task-1/move"
    );
}

#[test]
fn clears_the_completed_tasks_of_a_list() {
    let mut coroutine = GtasksTasksClear::new(&auth(), "list-1").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(request_line(&request), "POST /tasks/v1/lists/list-1/clear");
}
