//! Offline coverage of the transport every coroutine delegates to:
//! the authorized request builder, the path encoding, the entity tag
//! guard, the JSON parsing, the error envelope and its retry delay, the
//! redirect refusal and the keep-alive flag.

mod common;

use core::time::Duration;

use common::*;
use io_gtasks::v1::{
    rest::{
        tasklists::get::GtasksTaskListGet,
        tasks::{delete::GtasksTaskDelete, get::GtasksTaskGet},
    },
    send::{GTASKS_API_BASE, GtasksApiError, GtasksSendError, parse_api_error},
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
        let err = GtasksSendError::Api(GtasksApiError::parse(status, b"busy"));
        assert!(err.is_retryable(), "{status} should be retryable");
    }

    for status in [400, 401, 403, 404, 409, 412] {
        let err = GtasksSendError::Api(GtasksApiError::parse(status, b"nope"));
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

#[test]
fn refuses_an_id_the_path_would_drop() {
    for id in ["", ".", "..", ".\t.", "a\tb", "\n", "a\rb"] {
        assert!(
            matches!(
                GtasksTaskGet::new(&auth(), "list-1", id),
                Err(GtasksSendError::InvalidRequest(_))
            ),
            "task id {id:?} should be refused"
        );
        assert!(
            matches!(
                GtasksTaskGet::new(&auth(), id, "task-1"),
                Err(GtasksSendError::InvalidRequest(_))
            ),
            "task list id {id:?} should be refused"
        );
    }
}

#[test]
fn keeps_an_encoded_dot_segment_as_an_id() {
    let mut coroutine = GtasksTaskGet::new(&auth(), "list-1", "%2e%2e").unwrap();
    let (request, _) = expect_exchange(&mut coroutine, &json_response("200 OK", "{}"));

    assert_eq!(
        request_line(&request),
        "GET /tasks/v1/lists/list-1/tasks/%252e%252e"
    );
}

#[test]
fn refuses_an_entity_tag_holding_a_control_character() {
    for tag in ["\"a\"\r\nX-Injected: 1", "\"a\"\n", "\"a\"\0", "\"a\"\t"] {
        assert!(
            matches!(
                GtasksTaskDelete::new(&auth(), "list-1", "task-1", Some(tag)),
                Err(GtasksSendError::InvalidRequest(_))
            ),
            "entity tag {tag:?} should be refused"
        );
    }
}

#[test]
fn flags_a_rate_limit_reason_as_retryable() {
    let err = api_error(
        "403 Forbidden",
        &[],
        r#"{"error":{"code":403,"message":"Rate Limit Exceeded","errors":[{"domain":"usageLimits","reason":"rateLimitExceeded","message":"Rate Limit Exceeded"}],"status":"PERMISSION_DENIED","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"RATE_LIMIT_EXCEEDED"}]}}"#,
    );
    let api = err.api().unwrap();

    assert_eq!(api.reasons, ["rateLimitExceeded"]);
    assert_eq!(api.google_status.as_deref(), Some("PERMISSION_DENIED"));
    assert_eq!(api.detail_reasons, ["RATE_LIMIT_EXCEEDED"]);
    assert!(api.has_reason("rateLimitExceeded"));
    assert!(err.is_rate_limited());
    assert!(err.is_retryable());
}

#[test]
fn tells_a_rate_limit_from_another_refusal() {
    let error = |code: u16, reason: &str, status: &str| {
        GtasksApiError::parse(
            code,
            format!(
                r#"{{"error":{{"code":{code},"message":"no","errors":[{{"reason":"{reason}"}}],"status":"{status}"}}}}"#
            )
            .as_bytes(),
        )
    };

    assert!(error(403, "userRateLimitExceeded", "PERMISSION_DENIED").is_rate_limited());
    assert!(error(403, "quotaExceeded", "PERMISSION_DENIED").is_rate_limited());
    assert!(error(400, "badRequest", "RESOURCE_EXHAUSTED").is_rate_limited());
    assert!(error(429, "badRequest", "UNKNOWN").is_rate_limited());

    // NOTE: waiting does not lift the daily quota.
    let daily = error(403, "dailyLimitExceeded", "PERMISSION_DENIED");
    assert!(daily.has_reason("dailyLimitExceeded"));
    assert!(!daily.is_rate_limited());
    assert!(!daily.is_retryable());

    let denied = error(403, "insufficientPermissions", "PERMISSION_DENIED");
    assert!(!denied.is_rate_limited());

    let missing = error(404, "notFound", "NOT_FOUND");
    assert!(missing.is_not_found());
    assert!(!missing.is_retryable());
}

#[test]
fn carries_the_delay_of_the_retry_after_header() {
    let err = api_error("429 Too Many Requests", &[("Retry-After", "120")], "busy");

    assert_eq!(err.retry_after(), Some(Duration::from_secs(120)));
}

#[test]
fn carries_the_delay_of_the_envelope() {
    let body = r#"{"error":{"code":429,"message":"busy","status":"RESOURCE_EXHAUSTED","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"RATE_LIMIT_EXCEEDED"},{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"1.5s"}]}}"#;
    let err = api_error("429 Too Many Requests", &[], body);

    assert_eq!(err.retry_after(), Some(Duration::from_millis(1500)));
    assert_eq!(err.api().unwrap().detail_reasons, ["RATE_LIMIT_EXCEEDED"]);

    // NOTE: the header wins over the envelope.
    let err = api_error("429 Too Many Requests", &[("Retry-After", "7")], body);
    assert_eq!(err.retry_after(), Some(Duration::from_secs(7)));
}

#[test]
fn reads_no_delay_out_of_a_date_or_a_malformed_value() {
    for value in ["Fri, 13 Nov 2026 12:00:00 GMT", "-5", "+5", "soon", ""] {
        let err = api_error("503 Service Unavailable", &[("Retry-After", value)], "");
        assert_eq!(err.retry_after(), None, "got a delay out of {value:?}");
    }

    let err = api_error("503 Service Unavailable", &[], "");
    assert_eq!(err.retry_after(), None);

    // NOTE: a delay of another shape must not cost the envelope.
    let body =
        r#"{"error":{"code":429,"message":"busy","details":[{"retryDelay":{"seconds":30}}]}}"#;
    let err = api_error("400 Bad Request", &[], body);
    assert_eq!(err.status(), Some(429));
    assert_eq!(err.retry_after(), None);
}

#[test]
fn reports_whether_a_refusal_leaves_the_connection_reusable() {
    let err = api_error("404 Not Found", &[("Connection", "keep-alive")], "");
    assert!(err.api().unwrap().keep_alive);

    let err = api_error("404 Not Found", &[("Connection", "close")], "");
    assert!(!err.api().unwrap().keep_alive);
}

/// Runs an exchange answered by a non-2xx response and returns its
/// error.
fn api_error(status: &str, headers: &[(&str, &str)], body: &str) -> GtasksSendError {
    let response = http_response(status, headers, body);
    let mut coroutine = GtasksTaskListGet::new(&auth(), "@default").unwrap();
    let (_, ret) = expect_exchange(&mut coroutine, &response);

    match ret {
        Err(err) => err,
        Ok(_) => panic!("expected an API error"),
    }
}
