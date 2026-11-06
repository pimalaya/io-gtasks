//! Offline coverage of the standard blocking client: its methods run
//! against a scripted stream replaying canned HTTP responses, plus the
//! error paths a real stream can take.

#![cfg(feature = "client")]

mod common;

use std::{
    collections::VecDeque,
    io::{Error as IoError, Read, Result as IoResult, Write},
};

use common::*;
use io_gtasks::v1::{
    client::{GtasksClientStd, GtasksClientStdError},
    field::GtasksField,
    rest::{
        tasklists::GtasksTaskList,
        tasks::{GtasksTask, r#move::GtasksTaskMoveParams},
    },
};

/// Stream replaying canned HTTP responses: each read pops and serves
/// the next response whole; each write is recorded as one request so a
/// test can assert on the requests the client produced.
struct ScriptedStream {
    responses: VecDeque<Vec<u8>>,
    requests: Vec<String>,
}

impl ScriptedStream {
    fn new(responses: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            requests: Vec::new(),
        }
    }
}

impl Read for ScriptedStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let Some(response) = self.responses.pop_front() else {
            return Ok(0);
        };

        assert!(response.len() <= buf.len(), "scripted response too large");
        buf[..response.len()].copy_from_slice(&response);
        Ok(response.len())
    }
}

impl Write for ScriptedStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.requests
            .push(String::from_utf8_lossy(buf).into_owned());
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

/// Stream failing every read, to surface I/O errors out of the client.
struct FailingStream;

impl Read for FailingStream {
    fn read(&mut self, _: &mut [u8]) -> IoResult<usize> {
        Err(IoError::other("scripted read failure"))
    }
}

impl Write for FailingStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

/// Client whose stream serves the given responses in order.
fn client(responses: Vec<Vec<u8>>) -> GtasksClientStd {
    GtasksClientStd::new(ScriptedStream::new(responses), "fake-token")
}

/// The requests the client wrote to its scripted stream, in order.
fn requests(client: &mut GtasksClientStd) -> Vec<String> {
    let stream = client
        .stream
        .as_any_mut()
        .downcast_mut::<ScriptedStream>()
        .expect("the client still owns its scripted stream");

    stream.requests.clone()
}

/// The request line of each request the client wrote.
fn request_lines(client: &mut GtasksClientStd) -> Vec<String> {
    requests(client)
        .iter()
        .map(|request| String::from(request_line(request)))
        .collect()
}

#[test]
fn walks_a_task_list_lifecycle() {
    let list = r#"{"id":"list-1","title":"Groceries"}"#;
    let mut client = client(vec![
        json_response("200 OK", &format!(r#"{{"items":[{list}]}}"#)),
        json_response("200 OK", list),
        json_response("200 OK", list),
        json_response("200 OK", list),
        json_response("200 OK", list),
        empty_response("204 No Content"),
    ]);
    let tasklist = GtasksTaskList {
        title: GtasksField::Set(String::from("Groceries")),
        ..Default::default()
    };

    let listed = client.tasklists_list(&Default::default()).unwrap().response;
    assert_eq!(listed.items.len(), 1);

    client.tasklist_get("list-1").unwrap();
    client.tasklist_insert(&tasklist).unwrap();
    client.tasklist_update("list-1", &tasklist).unwrap();
    client.tasklist_patch("list-1", &tasklist).unwrap();
    client.tasklist_delete("list-1").unwrap();

    assert_eq!(
        request_lines(&mut client),
        [
            "GET /tasks/v1/users/@me/lists",
            "GET /tasks/v1/users/@me/lists/list-1",
            "POST /tasks/v1/users/@me/lists",
            "PUT /tasks/v1/users/@me/lists/list-1",
            "PATCH /tasks/v1/users/@me/lists/list-1",
            "DELETE /tasks/v1/users/@me/lists/list-1",
        ]
    );
}

#[test]
fn walks_a_task_lifecycle() {
    let task = r#"{"id":"task-1","title":"Buy milk"}"#;
    let mut client = client(vec![
        json_response("200 OK", &format!(r#"{{"items":[{task}]}}"#)),
        json_response("200 OK", task),
        json_response("200 OK", task),
        json_response("200 OK", task),
        json_response("200 OK", task),
        json_response("200 OK", task),
        empty_response("204 No Content"),
        empty_response("204 No Content"),
    ]);
    let body = GtasksTask {
        title: GtasksField::Set(String::from("Buy milk")),
        ..Default::default()
    };
    let to_other_list = GtasksTaskMoveParams {
        destination_tasklist: Some("list-2"),
        ..Default::default()
    };

    let listed = client
        .tasks_list("list-1", &Default::default())
        .unwrap()
        .response;
    assert_eq!(listed.items.len(), 1);

    client.task_get("list-1", "task-1").unwrap();
    client
        .task_insert("list-1", &body, Some("task-0"), None)
        .unwrap();
    client
        .task_update("list-1", "task-1", &body, Some("\"tag-1\""))
        .unwrap();
    client.task_patch("list-1", "task-1", &body, None).unwrap();
    client
        .task_move("list-1", "task-1", &to_other_list)
        .unwrap();
    client.task_delete("list-1", "task-1", None).unwrap();
    client.tasks_clear("list-1").unwrap();

    assert_eq!(
        request_lines(&mut client),
        [
            "GET /tasks/v1/lists/list-1/tasks",
            "GET /tasks/v1/lists/list-1/tasks/task-1",
            "POST /tasks/v1/lists/list-1/tasks?parent=task-0",
            "PUT /tasks/v1/lists/list-1/tasks/task-1",
            "PATCH /tasks/v1/lists/list-1/tasks/task-1",
            "POST /tasks/v1/lists/list-1/tasks/task-1/move?destinationTasklist=list-2",
            "DELETE /tasks/v1/lists/list-1/tasks/task-1",
            "POST /tasks/v1/lists/list-1/clear",
        ]
    );

    let guarded: Vec<_> = requests(&mut client)
        .into_iter()
        .filter(|request| request.contains("If-Match: \"tag-1\""))
        .collect();
    assert_eq!(guarded.len(), 1);
    assert!(guarded[0].starts_with("PUT "), "got: {guarded:?}");
}

#[test]
fn surfaces_an_api_error() {
    let body = r#"{"error":{"code":404,"message":"Not Found"}}"#;
    let mut client = client(vec![json_response("404 Not Found", body)]);

    let Err(err) = client.tasklist_get("missing") else {
        panic!("expected an API error");
    };

    assert!(
        matches!(&err, GtasksClientStdError::Send(send) if send.status() == Some(404)),
        "got: {err}"
    );
}

#[test]
fn surfaces_a_stream_failure() {
    let mut client = GtasksClientStd::new(FailingStream, "fake-token");

    let Err(err) = client.tasklist_get("@default") else {
        panic!("expected an I/O error");
    };

    assert!(matches!(err, GtasksClientStdError::Io(_)), "got: {err}");
}

#[test]
fn swaps_its_stream() {
    let mut client = GtasksClientStd::new(FailingStream, "fake-token");
    client.set_stream(ScriptedStream::new([json_response("200 OK", "{}")]));

    client
        .tasklist_get("@default")
        .expect("tasklist get after the swap");
}

#[test]
fn keeps_the_token_out_of_its_debug_output() {
    let client = client(vec![]);
    let debug = format!("{client:?}");

    assert!(debug.contains("GtasksClientStd"), "got: {debug}");
    assert!(!debug.contains("fake-token"), "got: {debug}");
}
