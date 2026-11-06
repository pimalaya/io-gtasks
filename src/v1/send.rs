//! HTTP/JSON transport every Google Tasks coroutine delegates to.
//!
//! Builds the authorized request and parses the JSON response, or the
//! Tasks error envelope on failure.
//!
//! Tasks API reference: <https://developers.google.com/workspace/tasks/reference/rest>.

use core::marker::PhantomData;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use io_http::{
    coroutine::*,
    rfc6750::bearer::HttpAuthBearer,
    rfc9110::{
        request::HttpRequest,
        send::{HttpSendOutput, HttpSendYield},
    },
    rfc9112::send::{Http11Send, Http11SendError},
};
use log::{debug, trace};
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use thiserror::Error;
use url::Url;

use crate::coroutine::*;

/// Base URL of the Google Tasks REST API v1.
pub const GTASKS_API_BASE: &str = "https://tasks.googleapis.com/tasks/v1/";

/// Unit marker deserialized from empty 2xx bodies (delete, clear).
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct GtasksNoResponse;

impl<'de> Deserialize<'de> for GtasksNoResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let _ = serde::de::IgnoredAny::deserialize(deserializer)?;
        Ok(Self)
    }
}

/// Errors that can occur during a Google Tasks exchange.
#[derive(Debug, Error)]
pub enum GtasksSendError {
    /// The underlying HTTP exchange failed.
    #[error("Tasks HTTP request failed: {0}")]
    Send(#[from] Http11SendError),
    /// The request body could not be serialized to JSON.
    #[error("Tasks request serialization failed: {0}")]
    SerializeRequest(#[source] serde_json::Error),
    /// The 2xx response body could not be parsed as JSON.
    #[error("Tasks response parsing failed: {0}")]
    ParseResponse(#[source] serde_json::Error),
    /// The request URL could not be built.
    #[error("Tasks URL parsing failed: {0}")]
    ParseUrl(#[from] url::ParseError),
    /// Tasks returned a non-2xx status with its error envelope.
    #[error("Tasks API returned HTTP {status}: {message}")]
    Api {
        /// The effective status code, from the envelope when present.
        status: u16,
        /// The error message, from the envelope or the raw body.
        message: String,
    },
    /// The server answered with a redirect, which is never followed.
    #[error("Tasks server returned an unexpected redirect")]
    UnexpectedRedirect,
}

impl GtasksSendError {
    /// Returns the HTTP status code when the error is an API error.
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Whether the error is transient (429 or 5xx) and worth retrying.
    pub fn is_retryable(&self) -> bool {
        matches!(self.status(), Some(429 | 500 | 502 | 503 | 504))
    }

    /// Whether the error is a failed entity tag guard (412), meaning
    /// the task moved since the etag the write carried was read.
    pub fn is_precondition_failed(&self) -> bool {
        matches!(self.status(), Some(412))
    }
}

/// Terminal value of a successful Google Tasks exchange.
#[derive(Clone, Debug)]
pub struct GtasksSendOutput<T> {
    /// The parsed 2xx response body.
    pub response: T,
    /// Whether the server allows reusing the TCP/TLS connection.
    pub keep_alive: bool,
}

/// I/O-free coroutine sending one authorized HTTP request and parsing
/// the JSON response into `T`.
pub struct GtasksSend<T> {
    state: State,
    _phantom: PhantomData<T>,
}

impl<T: DeserializeOwned> GtasksSend<T> {
    /// Builds a GET request against the given URL.
    pub fn get(auth: &HttpAuthBearer, url: Url) -> Self {
        Self::with_method(auth, "GET", url, None, Vec::new())
    }

    /// Builds a DELETE request against the given URL.
    pub fn delete(auth: &HttpAuthBearer, url: Url) -> Self {
        Self::with_method(auth, "DELETE", url, None, Vec::new())
    }

    /// Builds a DELETE request guarded by an entity tag.
    ///
    /// See [`Self::with_guard`] for what the guard does.
    pub fn delete_if_match(auth: &HttpAuthBearer, url: Url, if_match: Option<&str>) -> Self {
        Self::with_guard(auth, "DELETE", url, None, Vec::new(), if_match)
    }

    /// Builds a POST request with no body, for the custom verbs that
    /// take all their input from the query (clear, move).
    pub fn post_empty(auth: &HttpAuthBearer, url: Url) -> Self {
        Self::with_method(auth, "POST", url, Some("application/json"), Vec::new())
    }

    /// Builds a POST request with the given value as JSON body.
    pub fn post_json<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
    ) -> Result<Self, GtasksSendError> {
        let body = serde_json::to_vec(body).map_err(GtasksSendError::SerializeRequest)?;
        Ok(Self::with_method(
            auth,
            "POST",
            url,
            Some("application/json"),
            body,
        ))
    }

    /// Builds a PUT request with the given value as JSON body.
    pub fn put_json<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
    ) -> Result<Self, GtasksSendError> {
        let body = serde_json::to_vec(body).map_err(GtasksSendError::SerializeRequest)?;
        Ok(Self::with_method(
            auth,
            "PUT",
            url,
            Some("application/json"),
            body,
        ))
    }

    /// Builds a PUT request with the given value as JSON body, guarded
    /// by an entity tag.
    ///
    /// See [`Self::with_guard`] for what the guard does.
    pub fn put_json_if_match<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
        if_match: Option<&str>,
    ) -> Result<Self, GtasksSendError> {
        let body = serde_json::to_vec(body).map_err(GtasksSendError::SerializeRequest)?;
        Ok(Self::with_guard(
            auth,
            "PUT",
            url,
            Some("application/json"),
            body,
            if_match,
        ))
    }

    /// Builds a PATCH request with the given value as JSON body.
    pub fn patch_json<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
    ) -> Result<Self, GtasksSendError> {
        let body = serde_json::to_vec(body).map_err(GtasksSendError::SerializeRequest)?;
        Ok(Self::with_method(
            auth,
            "PATCH",
            url,
            Some("application/json"),
            body,
        ))
    }

    /// Builds a PATCH request with the given value as JSON body,
    /// guarded by an entity tag.
    ///
    /// See [`Self::with_guard`] for what the guard does.
    pub fn patch_json_if_match<B: Serialize>(
        auth: &HttpAuthBearer,
        url: Url,
        body: &B,
        if_match: Option<&str>,
    ) -> Result<Self, GtasksSendError> {
        let body = serde_json::to_vec(body).map_err(GtasksSendError::SerializeRequest)?;
        Ok(Self::with_guard(
            auth,
            "PATCH",
            url,
            Some("application/json"),
            body,
            if_match,
        ))
    }

    /// Builds a request with an arbitrary method, content type and body.
    pub fn with_method(
        auth: &HttpAuthBearer,
        method: &str,
        url: Url,
        content_type: Option<&str>,
        body: Vec<u8>,
    ) -> Self {
        Self::with_guard(auth, method, url, content_type, body, None)
    }

    /// Builds a request with an arbitrary method, content type, body
    /// and optional entity tag guard.
    ///
    /// The guard rides as an `If-Match` header carrying the etag a read
    /// returned, so the write only lands while the resource has not
    /// moved underneath it; a stale tag comes back as HTTP 412. An
    /// absent guard overwrites unconditionally. Task writes honour it,
    /// task list writes ignore it.
    pub fn with_guard(
        auth: &HttpAuthBearer,
        method: &str,
        url: Url,
        content_type: Option<&str>,
        body: Vec<u8>,
        if_match: Option<&str>,
    ) -> Self {
        let host = url.host_str().unwrap_or("localhost");

        let mut request = HttpRequest::get(url.clone())
            .header("Host", host)
            .header("Accept", "application/json")
            .header("Authorization", auth.to_authorization())
            .body(body);

        if let Some(content_type) = content_type {
            request = request.header("Content-Type", content_type);
        }

        if let Some(if_match) = if_match {
            request = request.header("If-Match", if_match);
        }

        request.method = method.into();

        debug!("prepare request to send");
        trace!("method: {method}");
        trace!("url: {url}");
        trace!("if_match: {if_match:?}");

        Self {
            state: State::Send(Http11Send::new(request)),
            _phantom: PhantomData,
        }
    }
}

impl<T: DeserializeOwned> GtasksCoroutine for GtasksSend<T> {
    type Yield = GtasksYield;
    type Return = Result<GtasksSendOutput<T>, GtasksSendError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> GtasksCoroutineState<Self::Yield, Self::Return> {
        match &mut self.state {
            State::Send(send) => match send.resume(arg) {
                HttpCoroutineState::Yielded(HttpSendYield::WantsRead) => {
                    GtasksCoroutineState::Yielded(GtasksYield::WantsRead)
                }
                HttpCoroutineState::Yielded(HttpSendYield::WantsWrite(bytes)) => {
                    GtasksCoroutineState::Yielded(GtasksYield::WantsWrite(bytes))
                }
                HttpCoroutineState::Yielded(HttpSendYield::WantsRedirect { .. }) => {
                    GtasksCoroutineState::Complete(Err(GtasksSendError::UnexpectedRedirect))
                }
                HttpCoroutineState::Complete(Err(err)) => {
                    GtasksCoroutineState::Complete(Err(err.into()))
                }
                HttpCoroutineState::Complete(Ok(HttpSendOutput {
                    response,
                    keep_alive,
                    ..
                })) => {
                    if response.status.is_success() {
                        // NOTE: an empty 2xx body (a delete, a clear) is
                        // normalised to `{}`: `null` would fail every
                        // struct response with "invalid type: null".
                        let body = if response.body.is_empty() {
                            b"{}".as_slice()
                        } else {
                            response.body.as_slice()
                        };

                        match serde_json::from_slice::<T>(body) {
                            Ok(response) => GtasksCoroutineState::Complete(Ok(GtasksSendOutput {
                                response,
                                keep_alive,
                            })),
                            Err(err) => GtasksCoroutineState::Complete(Err(
                                GtasksSendError::ParseResponse(err),
                            )),
                        }
                    } else {
                        let (status, message) = parse_api_error(*response.status, &response.body);
                        GtasksCoroutineState::Complete(Err(GtasksSendError::Api {
                            status,
                            message,
                        }))
                    }
                }
            },
        }
    }
}

enum State {
    Send(Http11Send),
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    code: Option<u16>,
    message: Option<String>,
}

/// Parses the Tasks JSON error envelope, falling back to the raw body;
/// returns the effective status code and message.
pub fn parse_api_error(http_status: u16, body: &[u8]) -> (u16, String) {
    if let Ok(envelope) = serde_json::from_slice::<ErrorEnvelope>(body) {
        let status = envelope.error.code.unwrap_or(http_status);
        let message = envelope
            .error
            .message
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| String::from("unknown Tasks API error"));
        return (status, message);
    }

    let message = String::from_utf8_lossy(body).trim().to_string();

    if message.is_empty() {
        (http_status, String::from("unknown Tasks API error"))
    } else {
        (http_status, message)
    }
}

/// Builds the URL of an API path from its segments.
///
/// Each segment is percent-encoded on its own, so an identifier can
/// never read as a path separator, a query or a scheme.
pub(crate) fn api_url<'a>(
    segments: impl IntoIterator<Item = &'a str>,
) -> Result<Url, GtasksSendError> {
    let mut url = Url::parse(GTASKS_API_BASE)?;

    // NOTE: the base is hierarchical, so its path is always mutable.
    if let Ok(mut path) = url.path_segments_mut() {
        path.pop_if_empty().extend(segments);
    }

    Ok(url)
}
