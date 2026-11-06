//! # iCalendar projection
//!
//! Projects a Google task onto an iCalendar VTODO and back. The Tasks
//! API exposes no iCalendar form of a task, so a consumer needing one (a
//! task client, a sync engine) synthesizes the document of record from
//! the JSON resource ([`GtasksTask::to_ical`]) and reads it back
//! ([`GtasksTask::from_ical`]).
//!
//! A task holds little, so the projection is short:
//!
//! | Task          | VTODO (RFC 5545 3.6.2)                               |
//! |---------------|------------------------------------------------------|
//! | `id`          | `UID`, written only                                  |
//! | `title`       | `SUMMARY`                                            |
//! | `notes`       | `DESCRIPTION`                                        |
//! | `status`      | `STATUS`, `NEEDS-ACTION` or `COMPLETED`              |
//! | `completed`   | `COMPLETED`                                          |
//! | `due`         | `DUE;VALUE=DATE`                                     |
//! | `parent`      | `RELATED-TO;RELTYPE=PARENT`                          |
//! | `updated`     | `DTSTAMP` and `LAST-MODIFIED`, written only          |
//! | `webViewLink` | `X-GOOGLE-WEB-VIEW-LINK`, written only               |
//! | `links`       | `LINK;LINKREL=related` (RFC 9253 6.1), written only   |
//!
//! Everything else a VTODO carries (`CATEGORIES`, `PRIORITY`, `DTSTART`,
//! `RRULE`, alarms, `X-` properties) has no field to land in and is
//! dropped on the way back: a task has nowhere to keep it.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use ical::{
    param::IcalParam,
    prop::{IcalProp, IcalPropKind},
    tree::{
        codec::Codec,
        cst::{IcalCst, IcalItem},
        error::IcalParseError,
        line::IcalLine,
        param::reltype::RELTYPE,
    },
    value::{IcalValue, datetime::IcalDateTime, text::IcalText},
};
use jiff::Timestamp;
use thiserror::Error;

use crate::v1::{
    field::GtasksField,
    rest::tasks::{GtasksTask, GtasksTaskStatus},
};

/// Product identifier the synthesized document carries.
const PRODID: &str = "-//Pimalaya//io-gtasks//EN";

/// The property minted from the read-only link to the task in the web
/// UI.
const WEB_VIEW_LINK_PROP: &str = "X-GOOGLE-WEB-VIEW-LINK";

impl GtasksTask {
    /// Projects a task onto a fresh VCALENDAR document holding one
    /// VTODO.
    pub fn to_ical(&self) -> String {
        let task = self;
        let mut vtodo = IcalCst::empty("VTODO");

        let uid = task.id.clone().unwrap_or_default();
        vtodo.push(IcalProp::text(IcalPropKind::Uid, vec![], uid));

        // NOTE: DTSTAMP is mandatory (RFC 5545 3.6.2) and a task carries
        // no field of its own for it, so the last modification time
        // stands in.
        if let Some(updated) = task.updated.as_deref().and_then(ical_utc) {
            vtodo.push(stamp_prop(IcalPropKind::DtStamp, updated.clone()));
            vtodo.push(stamp_prop(IcalPropKind::LastModified, updated));
        }

        for (kind, value) in [
            (IcalPropKind::Summary, task.title.as_deref()),
            (IcalPropKind::Description, task.notes.as_deref()),
        ] {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                vtodo.push(IcalProp::text(kind, vec![], value.to_string()));
            }
        }

        let status = match task.status.as_option() {
            Some(GtasksTaskStatus::Completed) => "COMPLETED",
            _ => "NEEDS-ACTION",
        };
        vtodo.push(IcalProp::text(IcalPropKind::Status, vec![], status));

        if let Some(completed) = task.completed.as_deref().and_then(ical_utc) {
            vtodo.push(stamp_prop(IcalPropKind::Completed, completed));
        }

        if let Some(due) = task.due.as_deref().and_then(ical_date) {
            vtodo.push(IcalProp {
                name: IcalPropKind::Due.into(),
                params: vec![IcalParam::Value("DATE".into())],
                value: IcalValue::Date(due.into()),
            });
        }

        if let Some(parent) = &task.parent {
            vtodo.push(IcalProp::text(
                IcalPropKind::RelatedTo,
                vec![IcalParam::RelType("PARENT".into())],
                parent.clone(),
            ));
        }

        if let Some(link) = &task.web_view_link {
            vtodo.push(IcalProp::text(WEB_VIEW_LINK_PROP, vec![], link.clone()));
        }

        for link in &task.links {
            let Some(uri) = link.link.as_deref().filter(|uri| !uri.is_empty()) else {
                continue;
            };

            let mut params = vec![
                IcalParam::Value("URI".into()),
                IcalParam::LinkRel("related".into()),
            ];

            if let Some(label) = link
                .description
                .as_deref()
                .filter(|label| !label.is_empty())
            {
                params.push(IcalParam::Label(label.to_string().into()));
            }

            vtodo.push(IcalProp {
                name: IcalPropKind::Link.into(),
                params,
                value: IcalValue::Uri(uri.to_string().into()),
            });
        }

        let mut calendar = IcalCst::v2();
        calendar.push(IcalProp::text(IcalPropKind::ProdId, vec![], PRODID));
        calendar.push_component(vtodo);
        calendar.to_string()
    }

    /// Projects the first VTODO of an iCalendar document onto a task.
    ///
    /// Every writable field a VTODO maps to comes back set, or null when
    /// the VTODO lacks it, and the others stay unset: the task patches
    /// what the projection manages, clears what the document dropped and
    /// leaves the rest alone. It inserts as is too. A `DUE` or
    /// `COMPLETED` that does not read as a date stays unset rather than
    /// clearing the server value.
    ///
    /// The UID is not read back, the caller addressing the task by the
    /// id it already holds. A `RELATED-TO` naming a parent lands in
    /// `parent`, which writes ignore: `tasks.move` moves a task.
    pub fn from_ical(contents: &[u8]) -> Result<Self, GtasksTaskIcalError> {
        let calendar = IcalCst::parse(contents).map_err(GtasksTaskIcalError::Parse)?;
        let vtodo = take_vtodo(&calendar)?;

        let mut summary = None;
        let mut description = None;
        let mut status = None;
        let mut completed = None;
        let mut due = GtasksField::Null;
        let mut parent = None;

        for item in &vtodo.items {
            let IcalItem::Prop(line) = item else {
                continue;
            };

            let Ok(kind) = line.name.get().parse::<IcalPropKind>() else {
                continue;
            };

            match kind {
                IcalPropKind::Summary => summary = Some(text(line)),
                IcalPropKind::Description => description = Some(text(line)),
                IcalPropKind::Status => status = Some(line.raw_value_str().trim().to_uppercase()),
                IcalPropKind::Completed => completed = Some(line.raw_value_str().into_owned()),
                // NOTE: an unreadable DUE leaves the server value alone
                // rather than clearing it.
                IcalPropKind::Due => {
                    due = rfc3339_date(&line.raw_value_str())
                        .map(GtasksField::Set)
                        .unwrap_or_default()
                }
                IcalPropKind::RelatedTo if is_parent(line) => parent = Some(text(line)),
                _ => {}
            }
        }

        // NOTE: a VTODO stating no STATUS is done once it says when.
        let done = match status.as_deref() {
            Some(status) => status == "COMPLETED",
            None => completed.is_some(),
        };

        let (status, completed) = if done {
            // NOTE: an unset completion time keeps the one the server
            // holds, or stamps the current time on a fresh completion.
            let completed = completed.as_deref().and_then(rfc3339_utc);
            let completed = completed.map(GtasksField::Set).unwrap_or_default();
            (GtasksTaskStatus::Completed, completed)
        } else {
            (GtasksTaskStatus::NeedsAction, GtasksField::Null)
        };

        Ok(Self {
            title: GtasksField::Set(summary.unwrap_or_default()),
            notes: GtasksField::set_or_null(description),
            status: GtasksField::Set(status),
            due,
            completed,
            parent,
            ..Default::default()
        })
    }
}

/// An iCalendar document that cannot become a task.
#[derive(Debug, Error)]
pub enum GtasksTaskIcalError {
    /// The document does not parse.
    #[error("Parse iCalendar: {0}")]
    Parse(#[source] IcalParseError),
    /// The document holds a component a task list does not model.
    #[error("A task list holds tasks only, not {0}")]
    NotATodo(String),
    /// The document holds no component at all.
    #[error("The iCalendar contents carry no VTODO")]
    NoTodo,
}

/// The VTODO of a parsed document: the document itself, or its first
/// VTODO.
///
/// A document carrying none is refused by the component name it does
/// carry.
fn take_vtodo<'a>(calendar: &'a IcalCst<'a>) -> Result<&'a IcalCst<'a>, GtasksTaskIcalError> {
    if is_named(calendar, "VTODO") {
        return Ok(calendar);
    }

    let components = || {
        calendar.items.iter().filter_map(|item| match item {
            IcalItem::Component(child) => Some(child.as_ref()),
            _ => None,
        })
    };

    if let Some(vtodo) = components().find(|child| is_named(child, "VTODO")) {
        return Ok(vtodo);
    }

    let found = components()
        .map(component_name)
        .find(|name| !name.eq_ignore_ascii_case("VTIMEZONE"));

    match found {
        Some(name) => Err(GtasksTaskIcalError::NotATodo(name.to_uppercase())),
        None => Err(GtasksTaskIcalError::NoTodo),
    }
}

/// Whether a `RELATED-TO` names the parent, its default relation.
fn is_parent(line: &IcalLine<'_>) -> bool {
    line.param::<RELTYPE>()
        .is_none_or(|reltype| reltype.eq_ignore_ascii_case("PARENT"))
}

/// The wire name of a component: the value of its BEGIN line.
fn component_name(component: &IcalCst<'_>) -> String {
    component
        .begin
        .as_ref()
        .map(|begin| begin.raw_value_str().trim().to_string())
        .unwrap_or_default()
}

/// Whether a component's BEGIN line names it `name`.
fn is_named(component: &IcalCst<'_>, name: &str) -> bool {
    component_name(component).eq_ignore_ascii_case(name)
}

/// The decoded text of a property line, escapes resolved.
fn text(line: &IcalLine<'_>) -> String {
    IcalText::decode(&line.value).0.into_owned()
}

/// A UTC date-time property.
fn stamp_prop(kind: IcalPropKind, stamp: String) -> IcalProp<'static> {
    IcalProp {
        name: kind.into(),
        params: Vec::new(),
        value: IcalValue::DateTime(IcalDateTime(stamp.into())),
    }
}

/// An RFC 3339 timestamp to the iCalendar UTC form `YYYYMMDDTHHMMSSZ`.
fn ical_utc(date_time: &str) -> Option<String> {
    let stamp = date_time.parse::<Timestamp>().ok()?;
    Some(stamp.strftime("%Y%m%dT%H%M%SZ").to_string())
}

/// An RFC 3339 timestamp to the iCalendar date `YYYYMMDD` of its UTC
/// day, the day the API keeps.
fn ical_date(date_time: &str) -> Option<String> {
    let stamp = date_time.parse::<Timestamp>().ok()?;
    Some(stamp.strftime("%Y%m%d").to_string())
}

/// The date an iCalendar `DUE` value starts with, as the midnight UTC
/// timestamp the API reads.
///
/// A date-time keeps the date it is written with, whatever its zone:
/// the API records no time.
fn rfc3339_date(value: &str) -> Option<String> {
    let date = value.trim().get(..8)?;

    if !date.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    Some(alloc::format!(
        "{}-{}-{}T00:00:00.000Z",
        &date[..4],
        &date[4..6],
        &date[6..]
    ))
}

/// An iCalendar UTC date-time `YYYYMMDDTHHMMSSZ` to RFC 3339.
///
/// A floating or zoned value is refused rather than guessed: RFC 5545
/// 3.8.2.1 wants `COMPLETED` in UTC.
fn rfc3339_utc(value: &str) -> Option<String> {
    let value = value.trim();
    let stamp = value.strip_suffix(['Z', 'z'])?;
    let (date, time) = stamp.split_once(['T', 't'])?;

    let digits = |part: &str, len| part.len() == len && part.bytes().all(|b| b.is_ascii_digit());

    if !digits(date, 8) || !digits(time, 6) {
        return None;
    }

    Some(alloc::format!(
        "{}-{}-{}T{}:{}:{}.000Z",
        &date[..4],
        &date[4..6],
        &date[6..],
        &time[..2],
        &time[2..4],
        &time[4..]
    ))
}

#[cfg(test)]
mod tests {
    use alloc::{string::String, vec};

    use serde_json::json;

    use crate::v1::{
        field::GtasksField,
        rest::tasks::{
            GtasksTask, GtasksTaskLink, GtasksTaskStatus,
            ical::{GtasksTaskIcalError, rfc3339_utc},
        },
    };

    fn task() -> GtasksTask {
        GtasksTask {
            id: Some(String::from("task-1")),
            etag: Some(String::from("\"tag-1\"")),
            title: GtasksField::Set(String::from("Buy milk, eggs")),
            updated: Some(String::from("2026-11-05T09:30:00.000Z")),
            parent: Some(String::from("task-0")),
            position: Some(String::from("00000000000000000000")),
            notes: GtasksField::Set(String::from("Two litres\nand a dozen")),
            status: GtasksField::Set(GtasksTaskStatus::Completed),
            due: GtasksField::Set(String::from("2026-11-10T00:00:00.000Z")),
            completed: GtasksField::Set(String::from("2026-11-06T07:52:55.094Z")),
            links: vec![
                GtasksTaskLink {
                    link_type: Some(String::from("email")),
                    description: Some(String::from("Groceries; list")),
                    link: Some(String::from("https://mail.google.com/mail/#all/abc")),
                },
                GtasksTaskLink {
                    link_type: Some(String::from("generic")),
                    description: Some(String::from("nowhere")),
                    link: None,
                },
            ],
            web_view_link: Some(String::from("https://tasks.google.com/task/abc")),
            ..Default::default()
        }
    }

    #[test]
    fn to_ical_projects_every_mapped_field() {
        let ical = task().to_ical().replace("\r\n ", "");

        for line in [
            "BEGIN:VCALENDAR\r\n",
            "BEGIN:VTODO\r\n",
            "UID:task-1\r\n",
            "DTSTAMP:20261105T093000Z\r\n",
            "LAST-MODIFIED:20261105T093000Z\r\n",
            "SUMMARY:Buy milk\\, eggs\r\n",
            "DESCRIPTION:Two litres\\nand a dozen\r\n",
            "STATUS:COMPLETED\r\n",
            "COMPLETED:20261106T075255Z\r\n",
            "DUE;VALUE=DATE:20261110\r\n",
            "RELATED-TO;RELTYPE=PARENT:task-0\r\n",
            "X-GOOGLE-WEB-VIEW-LINK:https://tasks.google.com/task/abc\r\n",
            "LINK;VALUE=URI;LINKREL=related;LABEL=\"Groceries; list\":https://mail.google.com/mail/#all/abc\r\n",
            "END:VTODO\r\n",
        ] {
            assert!(ical.contains(line), "missing {line:?} in:\n{ical}");
        }

        assert_eq!(
            ical.matches("\r\nLINK").count(),
            1,
            "a link without URL is skipped"
        );
    }

    #[test]
    fn to_ical_leaves_out_what_the_task_lacks() {
        let task = GtasksTask {
            id: Some(String::from("task-2")),
            title: GtasksField::Set(String::new()),
            notes: GtasksField::Null,
            status: GtasksField::Set(GtasksTaskStatus::NeedsAction),
            ..Default::default()
        };
        let ical = task.to_ical();

        assert!(ical.contains("STATUS:NEEDS-ACTION\r\n"), "{ical}");

        for name in [
            "SUMMARY",
            "DESCRIPTION",
            "DTSTAMP",
            "COMPLETED",
            "DUE",
            "RELATED-TO",
            "LINK",
        ] {
            assert!(
                !ical.contains(&alloc::format!("\n{name}")),
                "{name} in:\n{ical}"
            );
        }
    }

    #[test]
    fn from_ical_reads_back_what_to_ical_writes() {
        let source = task();
        let task = GtasksTask::from_ical(source.to_ical().as_bytes()).unwrap();

        assert_eq!(task.title, source.title);
        assert_eq!(task.notes, source.notes);
        assert_eq!(task.status, source.status);
        assert_eq!(task.due, source.due);
        assert_eq!(
            task.completed,
            GtasksField::Set(String::from("2026-11-06T07:52:55.000Z"))
        );
        assert_eq!(task.parent, source.parent);

        assert_eq!(task.id, None);
        assert_eq!(task.etag, None);
        assert_eq!(task.updated, None);
        assert_eq!(task.web_view_link, None);
        assert!(task.links.is_empty());
    }

    #[test]
    fn from_ical_clears_what_the_vtodo_dropped_and_leaves_the_rest_alone() {
        let ical = concat!(
            "BEGIN:VCALENDAR\r\n",
            "VERSION:2.0\r\n",
            "PRODID:-//Example//EN\r\n",
            "BEGIN:VTODO\r\n",
            "UID:local-1\r\n",
            "DTSTAMP:20261105T093000Z\r\n",
            "SUMMARY:Call back\r\n",
            "STATUS:NEEDS-ACTION\r\n",
            "CATEGORIES:@phone,next\r\n",
            "PRIORITY:1\r\n",
            "X-PIMALAYA-GTD:waiting\r\n",
            "BEGIN:VALARM\r\n",
            "ACTION:DISPLAY\r\n",
            "TRIGGER:-PT15M\r\n",
            "END:VALARM\r\n",
            "END:VTODO\r\n",
            "END:VCALENDAR\r\n",
        );
        let task = GtasksTask::from_ical(ical.as_bytes()).unwrap();

        assert_eq!(
            serde_json::to_value(&task).unwrap(),
            json!({
                "title": "Call back",
                "notes": null,
                "status": "needsAction",
                "due": null,
                "completed": null,
            })
        );
    }

    #[test]
    fn from_ical_keeps_the_date_a_due_date_time_is_written_with() {
        for due in [
            "DUE;TZID=America/New_York:20261110T233000",
            "DUE:20261110T233000Z",
            "DUE:20261110T233000",
            "DUE;VALUE=DATE:20261110",
        ] {
            let ical = alloc::format!("BEGIN:VTODO\r\nUID:a\r\n{due}\r\nEND:VTODO\r\n");
            let task = GtasksTask::from_ical(ical.as_bytes()).unwrap();

            assert_eq!(
                task.due,
                GtasksField::Set(String::from("2026-11-10T00:00:00.000Z")),
                "{due}"
            );
        }
    }

    #[test]
    fn from_ical_leaves_the_due_date_alone_when_it_cannot_read_it() {
        let ical = "BEGIN:VTODO\r\nUID:a\r\nDUE:next tuesday\r\nEND:VTODO\r\n";
        let task = GtasksTask::from_ical(ical.as_bytes()).unwrap();

        assert_eq!(task.due, GtasksField::Unset);
    }

    #[test]
    fn from_ical_reads_the_status_from_completed_alone() {
        let ical = "BEGIN:VTODO\r\nUID:a\r\nCOMPLETED:20261101T101500Z\r\nEND:VTODO\r\n";
        let task = GtasksTask::from_ical(ical.as_bytes()).unwrap();
        assert_eq!(task.status, GtasksField::Set(GtasksTaskStatus::Completed));
        assert_eq!(
            task.completed,
            GtasksField::Set(String::from("2026-11-01T10:15:00.000Z"))
        );

        let ical = "BEGIN:VTODO\r\nUID:a\r\nSTATUS:COMPLETED\r\nEND:VTODO\r\n";
        let task = GtasksTask::from_ical(ical.as_bytes()).unwrap();
        assert_eq!(task.status, GtasksField::Set(GtasksTaskStatus::Completed));
        assert_eq!(task.completed, GtasksField::Unset);

        for status in ["IN-PROCESS", "CANCELLED", "NEEDS-ACTION"] {
            let ical = alloc::format!(
                "BEGIN:VTODO\r\nUID:a\r\nSTATUS:{status}\r\nCOMPLETED:20261101T101500Z\r\nEND:VTODO\r\n"
            );
            let task = GtasksTask::from_ical(ical.as_bytes()).unwrap();
            assert_eq!(task.status, GtasksField::Set(GtasksTaskStatus::NeedsAction));
            assert_eq!(task.completed, GtasksField::Null);
        }
    }

    #[test]
    fn from_ical_reads_only_a_parent_relation() {
        let ical = concat!(
            "BEGIN:VTODO\r\n",
            "UID:a\r\n",
            "RELATED-TO;RELTYPE=CHILD:c\r\n",
            "RELATED-TO;RELTYPE=SIBLING:s\r\n",
            "END:VTODO\r\n",
        );
        let task = GtasksTask::from_ical(ical.as_bytes()).unwrap();
        assert_eq!(task.parent, None);

        let ical = "BEGIN:VTODO\r\nUID:a\r\nRELATED-TO:p\r\nEND:VTODO\r\n";
        let task = GtasksTask::from_ical(ical.as_bytes()).unwrap();
        assert_eq!(task.parent.as_deref(), Some("p"));
    }

    #[test]
    fn from_ical_reads_a_missing_summary_as_an_empty_title() {
        let task = GtasksTask::from_ical(b"BEGIN:VTODO\r\nUID:a\r\nEND:VTODO\r\n").unwrap();
        assert_eq!(task.title, GtasksField::Set(String::new()));
    }

    #[test]
    fn from_ical_refuses_a_document_without_a_vtodo() {
        let ical = concat!(
            "BEGIN:VCALENDAR\r\n",
            "VERSION:2.0\r\n",
            "BEGIN:VEVENT\r\n",
            "UID:a\r\n",
            "END:VEVENT\r\n",
            "END:VCALENDAR\r\n",
        );
        let err = GtasksTask::from_ical(ical.as_bytes()).unwrap_err();
        assert!(matches!(err, GtasksTaskIcalError::NotATodo(ref name) if name == "VEVENT"));

        let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n";
        let err = GtasksTask::from_ical(ical.as_bytes()).unwrap_err();
        assert!(matches!(err, GtasksTaskIcalError::NoTodo));

        let err = GtasksTask::from_ical(b"BEGIN:VTODO\r\n").unwrap_err();
        assert!(matches!(err, GtasksTaskIcalError::Parse(_)));
    }

    #[test]
    fn rfc3339_utc_refuses_a_floating_or_malformed_value() {
        assert_eq!(rfc3339_utc("20261101T101500"), None);
        assert_eq!(rfc3339_utc("20261101Z"), None);
        assert_eq!(rfc3339_utc("2026-11-01T10:15:00Z"), None);
    }
}
