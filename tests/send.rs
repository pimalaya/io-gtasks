//! Offline coverage of the transport every coroutine delegates to:
//! the authorized request builder, the path encoding, the entity tag
//! guard, the JSON parsing, the error envelope, the redirect refusal
//! and the keep-alive flag.

mod common;

use common::*;
use io_gtasks::v1::{
    rest::{
        tasklists::get::GtasksTaskListGet,
        tasks::{delete::GtasksTaskDelete, get::GtasksTaskGet},
    },
    send::{GTASKS_API_BASE, GtasksSendError, parse_api_error},
};

#[test]
fn targets_the_versioned_api_base() {
    assert_eq!(GTASKS_API_BASE, "https://tasks.googleapis.com/tasks/v1/");
}

#[test]
fn authorizes_every_request() {
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    ret.unwrap();

    assert!(
        request.contains("Authorization: Bearer fake-token"),
        "got: {request}"
    );
    assert!(
        request.contains("Accept: application/json"),
        "got: {request}"
    );
    assert!(
        request.contains("Host: tasks.googleapis.com"),
        "got: {request}"
    );
    assert!(!request.contains("If-Match"), "got: {request}");
}

#[test]
fn encodes_each_path_segment_on_its_own() {
    let mut coroutine = GtasksTaskGet::new(&auth(), "a/b?c#d", "x:y z").unwrap();
    let (request, _) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    assert_eq!(
        request_line(&request),
        "GET /tasks/v1/lists/a%2Fb%3Fc%23d/tasks/x:y%20z"
    );
}

#[test]
fn guards_a_write_with_the_given_entity_tag() {
    let mut coroutine =
        GtasksTaskDelete::new(&auth(), "list-1", "task-1", Some("\"tag-1\"")).unwrap();
    let (request, ret) = expect_exchange(&mut coroutine, &empty_response("204 No Content"));

    ret.unwrap();

    assert_eq!(
        request_line(&request),
        "DELETE /tasks/v1/lists/list-1/tasks/task-1"
    );
    assert!(request.contains("If-Match: \"tag-1\""), "got: {request}");
}

#[test]
fn recognises_a_failed_entity_tag_guard() {
    let response = json_response(
        "412 Precondition Failed",
        r#"{"error":{"code":412,"message":"Precondition Failed"}}"#,
    );
    let mut coroutine =
        GtasksTaskDelete::new(&auth(), "list-1", "task-1", Some("\"old\"")).unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected an API error");
    };

    assert!(err.is_precondition_failed());
    assert!(!err.is_retryable());
}

#[test]
fn reports_a_reusable_connection() {
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    assert!(ret.unwrap().keep_alive);
}

#[test]
fn reports_a_closed_connection() {
    let response = http_response("200 OK", &[("Connection", "close")], "{}");
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);

    assert!(!ret.unwrap().keep_alive);
}

#[test]
fn surfaces_the_error_envelope() {
    let response = json_response(
        "403 Forbidden",
        r#"{"error":{"code":403,"message":"Insufficient Permission","errors":[]}}"#,
    );
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected an API error");
    };

    assert_eq!(err.status(), Some(403));
    assert!(err.to_string().contains("Insufficient Permission"));
    assert!(!err.is_retryable());
    assert!(!err.is_precondition_failed());
}

#[test]
fn falls_back_to_the_raw_error_body() {
    let response = http_response("502 Bad Gateway", &[], "upstream exploded");
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected an API error");
    };

    assert_eq!(err.status(), Some(502));
    assert!(err.to_string().contains("upstream exploded"));
    assert!(err.is_retryable());
}

#[test]
fn refuses_to_follow_a_redirect() {
    let response = http_response(
        "302 Found",
        &[("Location", "https://elsewhere.example.org/")],
        "",
    );
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected a redirect refusal");
    };

    assert!(
        matches!(err, GtasksSendError::UnexpectedRedirect),
        "got: {err}"
    );
    assert_eq!(err.status(), None);
}

#[test]
fn surfaces_a_broken_http_reply() {
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, b"definitely not HTTP\r\n\r\n");
    let Err(err) = ret else {
        panic!("expected a transport error");
    };

    assert!(matches!(err, GtasksSendError::Send(_)), "got: {err}");
    assert_eq!(err.status(), None);
    assert!(!err.is_retryable());
}

#[test]
fn reports_an_unparsable_success_body() {
    let response = json_response("200 OK", "{not json");
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);
    let Err(err) = ret else {
        panic!("expected a parse error");
    };

    assert!(
        matches!(err, GtasksSendError::ParseResponse(_)),
        "got: {err}"
    );
}

#[test]
fn flags_the_transient_statuses_as_retryable() {
    for status in [429, 500, 502, 503, 504] {
        let err = GtasksSendError::Api {
            status,
            message: String::from("busy"),
        };
        assert!(err.is_retryable(), "{status} should be retryable");
    }

    for status in [400, 401, 403, 404, 409, 412] {
        let err = GtasksSendError::Api {
            status,
            message: String::from("nope"),
        };
        assert!(!err.is_retryable(), "{status} should not be retryable");
    }
}

#[test]
fn parses_error_bodies() {
    let (status, message) =
        parse_api_error(400, br#"{"error":{"code":403,"message":"Forbidden"}}"#);
    assert_eq!(status, 403);
    assert_eq!(message, "Forbidden");

    // NOTE: an envelope without a code keeps the HTTP status, and a
    // blank message falls back rather than surfacing as an empty error.
    let (status, message) = parse_api_error(418, br#"{"error":{"message":"   "}}"#);
    assert_eq!(status, 418);
    assert_eq!(message, "unknown Tasks API error");

    let (status, message) = parse_api_error(500, b"  boom  ");
    assert_eq!(status, 500);
    assert_eq!(message, "boom");

    let (status, message) = parse_api_error(503, b"");
    assert_eq!(status, 503);
    assert_eq!(message, "unknown Tasks API error");
}
