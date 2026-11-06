//! Offline coverage of the task lists (`tasklists`): the six methods
//! and the task list representation.

mod common;

use common::*;
use io_gtasks::v1::{
    field::GtasksField,
    rest::tasklists::{
        GtasksTaskList,
        delete::GtasksTaskListDelete,
        get::GtasksTaskListGet,
        insert::GtasksTaskListInsert,
        list::{GtasksTaskListsList, GtasksTaskListsListParams},
        patch::GtasksTaskListPatch,
        update::GtasksTaskListUpdate,
    },
};

const TASKLIST: &str = r#"{"kind":"tasks#taskList","id":"list-1","etag":"\"tag-1\"","title":"Groceries","updated":"2026-11-06T07:51:57.869Z","selfLink":"https://www.googleapis.com/tasks/v1/users/@me/lists/list-1"}"#;

fn titled(title: &str) -> GtasksTaskList {
    GtasksTaskList {
        title: GtasksField::Set(String::from(title)),
        ..Default::default()
    }
}

#[test]
fn lists_task_lists_page_by_page() {
    let params = GtasksTaskListsListParams {
        max_results: Some(2),
        page_token: Some("page-2"),
    };
    let body =
        format!(r#"{{"kind":"tasks#taskLists","items":[{TASKLIST}],"nextPageToken":"page-3"}}"#);
    let mut coroutine = GtasksTaskListsList::new(&auth(), &params).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", &body));
    let out = ret.unwrap();

    assert_eq!(out.response.items.len(), 1);
    assert_eq!(out.response.next_page_token.as_deref(), Some("page-3"));

    assert_eq!(
        request_line(&request),
        "GET /tasks/v1/users/@me/lists?maxResults=2&pageToken=page-2"
    );
}

#[test]
fn lists_task_lists_without_a_query() {
    let mut coroutine = GtasksTaskListsList::new(&auth(), &Default::default()).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    assert!(ret.unwrap().response.items.is_empty());
    assert_eq!(request_line(&request), "GET /tasks/v1/users/@me/lists");
}

#[test]
fn gets_the_default_task_list() {
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", TASKLIST));
    let out = ret.unwrap();

    assert_eq!(out.response.id.as_deref(), Some("list-1"));
    assert_eq!(
        out.response.title,
        GtasksField::Set(String::from("Groceries"))
    );
    assert_eq!(
        out.response.updated.as_deref(),
        Some("2026-11-06T07:51:57.869Z")
    );

    assert_eq!(
        request_line(&request),
        "GET /tasks/v1/users/@me/lists/@default"
    );
}

#[test]
fn inserts_a_task_list() {
    let mut coroutine = GtasksTaskListInsert::new(&auth(), &titled("Groceries")).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", TASKLIST));

    assert_eq!(ret.unwrap().response.id.as_deref(), Some("list-1"));
    assert_eq!(request_line(&request), "POST /tasks/v1/users/@me/lists");
    assert_eq!(request_body(&request), r#"{"title":"Groceries"}"#);
}

#[test]
fn updates_a_task_list() {
    let tasklist = GtasksTaskList {
        id: Some(String::from("list-1")),
        ..titled("Errands")
    };
    let mut coroutine = GtasksTaskListUpdate::new(&auth(), "list-1", &tasklist).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", TASKLIST));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "PUT /tasks/v1/users/@me/lists/list-1"
    );
    assert_eq!(
        request_body(&request),
        r#"{"id":"list-1","title":"Errands"}"#
    );
}

#[test]
fn patches_a_task_list() {
    let mut coroutine = GtasksTaskListPatch::new(&auth(), "list-1", &titled("Errands")).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", TASKLIST));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "PATCH /tasks/v1/users/@me/lists/list-1"
    );
    assert_eq!(request_body(&request), r#"{"title":"Errands"}"#);
}

#[test]
fn deletes_a_task_list() {
    let mut coroutine = GtasksTaskListDelete::new(&auth(), "list-1").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "DELETE /tasks/v1/users/@me/lists/list-1"
    );
    assert_eq!(request_body(&request), "");
}
