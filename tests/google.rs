//! Live tests against the Google Tasks API.
//!
//! Google only accepts an OAuth2 Bearer token (no app passwords), and
//! there are two ways to hand one to these tests.
//!
//! A token from the authorization-code flow, minted by hand, acts as
//! you and dies within the hour:
//!
//! ```sh
//! GTASKS_ACCESS_TOKEN="ya29...." \
//! cargo test --features ical --test google -- --ignored
//! ```
//!
//! A service account instead signs its own assertion and trades it for
//! a token whenever one is needed, which is what makes an unattended
//! run possible. Point the tests at its JSON key and they mint the
//! token themselves through io-oauth, so no token is ever handled by a
//! shell, written to a file or passed between CI steps:
//!
//! ```sh
//! GTASKS_SERVICE_ACCOUNT_KEY_FILE=key.json \
//! cargo test --features ical --test google -- --ignored
//! ```
//!
//! CI passes the key itself rather than a path, as
//! `GTASKS_SERVICE_ACCOUNT_KEY`, since it comes straight out of a
//! secret.
//!
//! Each flow runs twice: as the service account itself, on the task
//! lists it owns, and as a Workspace user the service account acts for
//! through domain-wide delegation, `GTASKS_SERVICE_ACCOUNT_SUBJECT`
//! (`google@pimalaya.org` by default, the Pimalaya test user). A token
//! minted by hand serves both passes. No task list the run did not
//! create is ever read or written, whichever token is used.
//!
//! [`account`] only reads the task lists and the default one.
//! [`tasks`] walks the whole surface on two throwaway task lists, and
//! [`with_cleanup`] deletes them however the run ends, so a failure
//! leaves the account with stray lists at worst. With the `ical`
//! feature, [`ical`] writes a task from a VTODO and patches it from an
//! edited one.

#![cfg(any(
    feature = "rustls-ring",
    feature = "rustls-aws",
    feature = "native-tls"
))]

use core::fmt::Debug;

use std::{
    borrow::Cow,
    env, fs,
    panic::{self, AssertUnwindSafe},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use io_gtasks::v1::{
    client::{GtasksClientStd, GtasksClientStdConnectOptions, GtasksClientStdError},
    field::GtasksField,
    rest::{
        tasklists::GtasksTaskList,
        tasks::{
            GtasksTask, GtasksTaskStatus, list::GtasksTasksListParams, r#move::GtasksTaskMoveParams,
        },
    },
};
use io_oauth::{
    client::Oauth20ClientStd,
    rfc7523::{
        assertion::{Oauth20JwtBearerClaims, Oauth20JwtBearerKey},
        auth_grant::Oauth20JwtBearerGrantRequestParams,
    },
};
use pimalaya_stream::tls::Tls;
use secrecy::ExposeSecret;
use serde::Deserialize;
use url::Url;

/// The scope the live tests need: read and write on the task lists.
const TASKS_SCOPE: &str = "https://www.googleapis.com/auth/tasks";

/// The Pimalaya Workspace user the service account acts as when
/// `GTASKS_SERVICE_ACCOUNT_SUBJECT` names none.
const DEFAULT_SUBJECT: &str = "google@pimalaya.org";

/// Read-only pass as the service account.
#[test]
#[ignore = "requires GTASKS_ACCESS_TOKEN env var and --ignored"]
fn account() {
    account_flow(&mut connect(Principal::ServiceAccount));
}

/// Read-only pass as the delegated user.
#[test]
#[ignore = "requires GTASKS_ACCESS_TOKEN env var and --ignored"]
fn delegated_account() {
    account_flow(&mut connect(Principal::Delegated));
}

/// Full pass as the service account.
#[test]
#[ignore = "requires GTASKS_ACCESS_TOKEN env var and --ignored"]
fn tasks() {
    tasks_flow(&mut connect(Principal::ServiceAccount));
}

/// Full pass as the delegated user.
#[test]
#[ignore = "requires GTASKS_ACCESS_TOKEN env var and --ignored"]
fn delegated_tasks() {
    tasks_flow(&mut connect(Principal::Delegated));
}

/// A task written from a VTODO reads back as the same VTODO, and a
/// patch from an edited one clears what the edit dropped without
/// touching what the projection does not manage.
#[cfg(feature = "ical")]
#[test]
#[ignore = "requires GTASKS_ACCESS_TOKEN env var and --ignored"]
fn ical() {
    let mut client = connect(Principal::ServiceAccount);
    let list_id = tasklist_create(&mut client, &format!("io-gtasks-test-{}", unix_millis()));

    with_cleanup(
        &mut client,
        |client| {
            let parent = retry("task insert", || {
                client.task_insert(&list_id, &titled("Groceries"), None, None)
            })
            .response;
            let parent_id = parent.id.clone().expect("the inserted task carries an id");

            let vtodo = concat!(
                "BEGIN:VCALENDAR\r\n",
                "VERSION:2.0\r\n",
                "PRODID:-//Example//EN\r\n",
                "BEGIN:VTODO\r\n",
                "UID:local-1@example.org\r\n",
                "DTSTAMP:20261105T093000Z\r\n",
                "SUMMARY:Buy milk\\, eggs\r\n",
                "DESCRIPTION:Two litres\\nand a dozen\r\n",
                "DUE;TZID=America/New_York:20261110T233000\r\n",
                "STATUS:NEEDS-ACTION\r\n",
                "CATEGORIES:@errands\r\n",
                "PRIORITY:1\r\n",
                "END:VTODO\r\n",
                "END:VCALENDAR\r\n",
            );
            let task = GtasksTask::from_ical(vtodo.as_bytes()).expect("parse the VTODO");
            let inserted = retry("task insert", || {
                client.task_insert(&list_id, &task, Some(&parent_id), None)
            })
            .response;
            let task_id = inserted
                .id
                .clone()
                .expect("the inserted task carries an id");

            assert_eq!(inserted.title.as_deref(), Some("Buy milk, eggs"));
            assert_eq!(inserted.notes.as_deref(), Some("Two litres\nand a dozen"));
            assert_eq!(inserted.due.as_deref(), Some("2026-11-10T00:00:00.000Z"));
            assert_eq!(inserted.parent.as_deref(), Some(parent_id.as_str()));

            let ical = inserted.to_ical().replace("\r\n ", "");
            assert!(ical.contains(&format!("UID:{task_id}\r\n")), "{ical}");
            assert!(ical.contains("DUE;VALUE=DATE:20261110\r\n"), "{ical}");
            assert!(
                ical.contains(&format!("RELATED-TO;RELTYPE=PARENT:{parent_id}\r\n")),
                "{ical}"
            );

            let edited = ical
                .replace("DESCRIPTION:Two litres\\nand a dozen\r\n", "")
                .replace("STATUS:NEEDS-ACTION\r\n", "STATUS:COMPLETED\r\n");
            let patch = GtasksTask::from_ical(edited.as_bytes()).expect("parse the edit");
            let etag = inserted.etag.as_deref();
            let patched = retry("task patch", || {
                client.task_patch(&list_id, &task_id, &patch, etag)
            })
            .response;

            assert_eq!(
                patched.notes,
                GtasksField::Unset,
                "the dropped notes are cleared"
            );
            assert_eq!(
                patched.status.as_option(),
                Some(&GtasksTaskStatus::Completed)
            );
            assert!(
                patched.completed.as_option().is_some(),
                "Google stamps the completion"
            );
            assert_eq!(patched.due, inserted.due);
            assert_eq!(patched.parent.as_deref(), Some(parent_id.as_str()));
        },
        |client| tasklist_delete(client, &list_id),
    );
}

fn account_flow(client: &mut GtasksClientStd) {
    let lists = retry("tasklists list", || {
        client.tasklists_list(&Default::default())
    })
    .response;

    assert_eq!(lists.kind.as_deref(), Some("tasks#taskLists"));
    assert!(
        !lists.items.is_empty(),
        "every principal has a default list"
    );

    let default = retry("default tasklist get", || client.tasklist_get("@default")).response;
    let ids: Vec<_> = lists.items.iter().filter_map(|l| l.id.as_deref()).collect();
    assert!(ids.contains(&default.id.as_deref().expect("the default list has an id")));
}

fn tasks_flow(client: &mut GtasksClientStd) {
    let title = format!("io-gtasks-test-{}", unix_millis());
    let list_id = tasklist_create(client, &title);
    let other_id = tasklist_create(client, &format!("{title}-other"));

    with_cleanup(
        client,
        |client| {
            tasklist_metadata(client, &list_id, &title);
            tasks_writes(client, &list_id, &other_id);
        },
        |client| {
            tasklist_delete(client, &list_id);
            tasklist_delete(client, &other_id);
        },
    );
}

fn tasklist_create(client: &mut GtasksClientStd, title: &str) -> String {
    let list = GtasksTaskList {
        title: GtasksField::Set(String::from(title)),
        ..Default::default()
    };

    retry("tasklist insert", || client.tasklist_insert(&list))
        .response
        .id
        .expect("the inserted task list carries an id")
}

fn tasklist_delete(client: &mut GtasksClientStd, id: &str) {
    if let Err(err) = client.tasklist_delete(id) {
        report_leftover("task list", id, &err);
    }
}

fn tasklist_metadata(client: &mut GtasksClientStd, id: &str, title: &str) {
    let fetched = retry("tasklist get", || client.tasklist_get(id)).response;
    assert_eq!(fetched.title.as_deref(), Some(title));

    let renamed = format!("{title}-patched");
    let patch = GtasksTaskList {
        title: GtasksField::Set(renamed.clone()),
        ..Default::default()
    };
    let patched = retry("tasklist patch", || client.tasklist_patch(id, &patch)).response;
    assert_eq!(patched.title.as_deref(), Some(renamed.as_str()));

    let update = GtasksTaskList {
        id: Some(String::from(id)),
        title: GtasksField::Set(String::from(title)),
        ..Default::default()
    };
    let updated = retry("tasklist update", || client.tasklist_update(id, &update)).response;
    assert_eq!(updated.title.as_deref(), Some(title));

    let lists = retry("tasklists list", || {
        client.tasklists_list(&Default::default())
    })
    .response;
    assert!(lists.items.iter().any(|l| l.id.as_deref() == Some(id)));
}

fn tasks_writes(client: &mut GtasksClientStd, list_id: &str, other_id: &str) {
    let parent = GtasksTask {
        notes: GtasksField::Set(String::from("parent notes")),
        due: GtasksField::Set(String::from("2026-11-10T23:30:00-05:00")),
        ..titled("parent")
    };
    let parent = retry("task insert", || {
        client.task_insert(list_id, &parent, None, None)
    })
    .response;
    let parent_id = parent.id.clone().expect("the inserted task carries an id");

    assert_eq!(
        parent.due.as_deref(),
        Some("2026-11-11T00:00:00.000Z"),
        "the due date keeps its UTC day, at midnight"
    );

    let child = retry("subtask insert", || {
        client.task_insert(list_id, &titled("child"), Some(&parent_id), None)
    })
    .response;
    let child_id = child.id.clone().expect("the inserted task carries an id");
    assert_eq!(child.parent.as_deref(), Some(parent_id.as_str()));

    let sibling = retry("sibling insert", || {
        client.task_insert(list_id, &titled("sibling"), None, Some(&parent_id))
    })
    .response;
    let sibling_id = sibling.id.clone().expect("the inserted task carries an id");
    assert!(
        sibling.position > parent.position,
        "inserted after its previous sibling"
    );

    // NOTE: a null clears, an unset field is left alone, and the
    // read-only parent rides along without effect.
    let patch = GtasksTask {
        notes: GtasksField::Null,
        due: GtasksField::Null,
        parent: Some(sibling_id.clone()),
        ..Default::default()
    };
    let patched = retry("task patch", || {
        client.task_patch(list_id, &parent_id, &patch, None)
    })
    .response;
    assert_eq!(patched.title.as_deref(), Some("parent"));
    assert_eq!(patched.notes, GtasksField::Unset);
    assert_eq!(patched.due, GtasksField::Unset);
    assert_eq!(patched.parent, None);

    // NOTE: a stale etag is refused, a current one lets the write land.
    let stale = patched.etag.clone();
    let renamed = retry("task update", || {
        client.task_update(list_id, &parent_id, &titled("parent renamed"), None)
    })
    .response;

    let err = client
        .task_patch(list_id, &parent_id, &titled("stale"), stale.as_deref())
        .expect_err("a stale etag is refused");
    assert!(precondition_failed(&err), "{err:?}");

    let err = client
        .task_delete(list_id, &parent_id, stale.as_deref())
        .expect_err("a stale etag is refused");
    assert!(precondition_failed(&err), "{err:?}");

    let current = renamed.etag.as_deref();
    let renamed = retry("guarded task patch", || {
        client.task_patch(list_id, &parent_id, &titled("parent"), current)
    })
    .response;

    // NOTE: completing the parent completes its subtask, whose update
    // time moves so an incremental listing returns it.
    let before = retry("task get", || client.task_get(list_id, &child_id)).response;
    let done = GtasksTask {
        status: GtasksField::Set(GtasksTaskStatus::Completed),
        ..Default::default()
    };
    retry("task complete", || {
        client.task_patch(list_id, &parent_id, &done, None)
    });
    let child = retry("task get", || client.task_get(list_id, &child_id)).response;
    assert_eq!(child.status.as_option(), Some(&GtasksTaskStatus::Completed));
    assert!(child.completed.as_option().is_some());
    assert!(
        child.updated > before.updated,
        "the cascade moves the update time"
    );

    let undone = GtasksTask {
        status: GtasksField::Set(GtasksTaskStatus::NeedsAction),
        ..Default::default()
    };
    let reopened = retry("task reopen", || {
        client.task_patch(list_id, &parent_id, &undone, None)
    })
    .response;
    assert_eq!(
        reopened.completed,
        GtasksField::Unset,
        "reopening drops the time"
    );

    // NOTE: a deletion is a flag an incremental listing returns.
    let since = renamed
        .updated
        .clone()
        .expect("a task carries its update time");
    retry("task delete", || {
        client.task_delete(list_id, &sibling_id, None)
    });

    let incremental = GtasksTasksListParams {
        updated_min: Some(&since),
        show_deleted: true,
        show_hidden: true,
        ..Default::default()
    };
    let changed = retry("tasks list", || client.tasks_list(list_id, &incremental)).response;
    let deleted = changed
        .items
        .iter()
        .find(|task| task.id.as_deref() == Some(sibling_id.as_str()))
        .expect("the incremental listing returns the deletion");
    assert_eq!(deleted.deleted, GtasksField::Set(true));

    let listed = retry("tasks list", || {
        client.tasks_list(list_id, &Default::default())
    })
    .response;
    assert!(
        listed
            .items
            .iter()
            .all(|t| t.id.as_deref() != Some(&sibling_id)),
        "a default listing hides deleted tasks"
    );

    // NOTE: moving to another list keeps the id and leaves no
    // tombstone behind.
    let moved = retry("task move", || {
        client.task_move(
            list_id,
            &child_id,
            &GtasksTaskMoveParams {
                destination_tasklist: Some(other_id),
                ..Default::default()
            },
        )
    })
    .response;
    assert_eq!(moved.id.as_deref(), Some(child_id.as_str()));
    assert_eq!(moved.parent, None);

    let everything = GtasksTasksListParams {
        show_deleted: true,
        show_hidden: true,
        ..Default::default()
    };
    let source = retry("tasks list", || client.tasks_list(list_id, &everything)).response;
    assert!(
        source
            .items
            .iter()
            .all(|t| t.id.as_deref() != Some(&child_id))
    );

    // NOTE: clearing hides the completed tasks rather than deleting
    // them.
    retry("tasks clear", || client.tasks_clear(other_id));
    let cleared = retry("tasks list", || client.tasks_list(other_id, &everything)).response;
    let hidden = cleared
        .items
        .iter()
        .find(|task| task.id.as_deref() == Some(child_id.as_str()))
        .expect("a hidden task stays listable");
    assert_eq!(hidden.hidden, Some(true));
    assert_eq!(hidden.deleted, GtasksField::Unset);

    // NOTE: deleting the parent deletes its subtasks.
    let child = retry("subtask insert", || {
        client.task_insert(list_id, &titled("child 2"), Some(&parent_id), None)
    })
    .response;
    let child_id = child.id.clone().expect("the inserted task carries an id");
    retry("task delete", || {
        client.task_delete(list_id, &parent_id, None)
    });
    let child = retry("task get", || client.task_get(list_id, &child_id)).response;
    assert_eq!(child.deleted, GtasksField::Set(true));
}

/// A bare task carrying a title.
fn titled(title: &str) -> GtasksTask {
    GtasksTask {
        title: GtasksField::Set(String::from(title)),
        ..Default::default()
    }
}

fn precondition_failed(err: &GtasksClientStdError) -> bool {
    matches!(err, GtasksClientStdError::Send(err) if err.is_precondition_failed())
}

/// Who the run acts as.
#[derive(Clone, Copy, Debug)]
enum Principal {
    /// The service account itself, on the task lists it owns.
    ServiceAccount,
    /// The Workspace user the service account acts for.
    Delegated,
}

fn connect(principal: Principal) -> GtasksClientStd {
    env_logger::try_init().ok();

    GtasksClientStd::connect(token(principal), GtasksClientStdConnectOptions::default())
        .expect("connect")
}

/// The bearer token the run authenticates with.
///
/// `GTASKS_ACCESS_TOKEN` short-circuits everything when it is set, for
/// the token you minted by hand. Otherwise a service account key, held
/// inline in `GTASKS_SERVICE_ACCOUNT_KEY` or at the path
/// `GTASKS_SERVICE_ACCOUNT_KEY_FILE`, is traded for a fresh token,
/// acting as `GTASKS_SERVICE_ACCOUNT_SUBJECT` for a delegated principal.
fn token(principal: Principal) -> String {
    if let Ok(token) = env::var("GTASKS_ACCESS_TOKEN") {
        return token;
    }

    let Some(key) = service_account_key() else {
        panic!(
            "set GTASKS_ACCESS_TOKEN, or GTASKS_SERVICE_ACCOUNT_KEY / \
             GTASKS_SERVICE_ACCOUNT_KEY_FILE to mint one"
        );
    };

    match principal {
        Principal::ServiceAccount => mint_token(&key, None),
        Principal::Delegated => mint_token(&key, Some(subject())),
    }
}

/// The Workspace user a delegated principal acts as.
fn subject() -> String {
    env::var("GTASKS_SERVICE_ACCOUNT_SUBJECT").unwrap_or_else(|_| String::from(DEFAULT_SUBJECT))
}

/// The service account key, inline or read from its path.
fn service_account_key() -> Option<String> {
    if let Some(key) = env::var("GTASKS_SERVICE_ACCOUNT_KEY")
        .ok()
        .filter(|key| !key.is_empty())
    {
        return Some(key);
    }

    let path = env::var("GTASKS_SERVICE_ACCOUNT_KEY_FILE").ok()?;
    let key = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read the service account key at {path}: {err}"));

    Some(key)
}

/// The subset of a service account key file the JWT bearer grant needs.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    #[serde(default = "default_token_uri")]
    token_uri: String,
}

fn default_token_uri() -> String {
    String::from("https://oauth2.googleapis.com/token")
}

/// Signs a JWT bearer assertion with the service account key and trades
/// it for an access token (RFC 7523 section 2.1), acting as `subject`
/// when one is given and as the service account itself otherwise.
fn mint_token(key: &str, subject: Option<String>) -> String {
    let key: ServiceAccountKey =
        serde_json::from_str(key).expect("the service account key is valid JSON");

    let signer = Oauth20JwtBearerKey::from_pkcs8_pem(&key.private_key)
        .expect("the service account key holds a PKCS#8 private key");

    let token_uri: Url = key.token_uri.parse().expect("the token URI is a valid URL");

    let mut client =
        Oauth20ClientStd::connect(token_uri, &Tls::default(), key.client_email.as_str())
            .expect("connect to the token endpoint");

    let claims = Oauth20JwtBearerClaims {
        iss: key.client_email.as_str().into(),
        sub: subject.map(Cow::from),
        scope: [Cow::from(TASKS_SCOPE)].into_iter().collect(),
        ..Default::default()
    };

    // NOTE: iat and exp come from the clock here, in the std client;
    // the coroutine layer underneath stays clock-free.
    let assertion = client
        .sign_jwt_bearer_assertion(&signer, claims, None, Duration::from_secs(600))
        .expect("sign the assertion");

    let params = Oauth20JwtBearerGrantRequestParams {
        assertion,
        scope: Default::default(),
    };

    let response = client
        .request_jwt_bearer_grant(params)
        .expect("trade the assertion for an access token");

    match response {
        Ok(granted) => granted.access_token.expose_secret().to_owned(),
        Err(err) => panic!("the token endpoint refused the assertion: {err:?}"),
    }
}

/// Milliseconds since the Unix epoch, to mint a unique list name per
/// run.
fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

/// Runs `body`, then `cleanup` whichever way `body` went, and only then
/// re-raises a panic `body` may have raised.
///
/// These flows run against a real account and every step panics on
/// failure, so a teardown written as the last statements of a flow
/// would be skipped the moment anything goes wrong.
fn with_cleanup<B, C>(client: &mut GtasksClientStd, body: B, cleanup: C)
where
    B: FnOnce(&mut GtasksClientStd),
    C: FnOnce(&mut GtasksClientStd),
{
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| body(client)));

    if panic::catch_unwind(AssertUnwindSafe(|| cleanup(client))).is_err() {
        eprintln!("WARNING: cleanup itself failed, the account may hold leftovers");
    }

    if let Err(payload) = outcome {
        panic::resume_unwind(payload);
    }
}

fn report_leftover(what: &str, id: &str, err: &dyn Debug) {
    eprintln!("WARNING: could not clean up {what} `{id}`, remove it by hand: {err:?}");
}

/// Runs a call, retrying with a growing delay while Google rate limits
/// it, and panics on any other error.
fn retry<T>(what: &str, mut call: impl FnMut() -> Result<T, GtasksClientStdError>) -> T {
    let mut delay = Duration::from_secs(2);

    for attempt in 1..=6 {
        match call() {
            Ok(out) => return out,
            Err(err) if attempt < 6 && rate_limited(&err) => {
                eprintln!("{what}: rate limited, retrying in {delay:?}");
                thread::sleep(delay);
                delay *= 2;
            }
            Err(err) => panic!("{what}: {err:?}"),
        }
    }

    unreachable!("the last attempt either returns or panics")
}

fn rate_limited(err: &GtasksClientStdError) -> bool {
    let GtasksClientStdError::Send(err) = err else {
        return false;
    };

    err.is_retryable()
        || (err.status() == Some(403) && {
            let message = err.to_string().to_lowercase();
            message.contains("rate limit") || message.contains("usage limits")
        })
}
