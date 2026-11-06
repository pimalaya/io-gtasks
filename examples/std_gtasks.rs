//! Full std client: list the open tasks of the default task list.
//!
//! Opens the TCP and TLS connection to the Tasks API, lists the tasks
//! of the default list still to be done, and prints each one. Needs an
//! OAuth 2.0 access token with the tasks.readonly scope, read from the
//! `GTASKS_ACCESS_TOKEN` environment variable:
//!
//! ```sh
//! GTASKS_ACCESS_TOKEN="<token>" cargo run --example std_gtasks
//! ```

use std::env;

use io_gtasks::v1::{client::GtasksClientStd, rest::tasks::list::GtasksTasksListParams};

fn main() {
    env_logger::try_init().ok();

    let token = env::var("GTASKS_ACCESS_TOKEN").expect("GTASKS_ACCESS_TOKEN not set");

    let mut client = GtasksClientStd::connect(token, Default::default()).unwrap();

    let params = GtasksTasksListParams {
        max_results: Some(100),
        show_completed: Some(false),
        ..Default::default()
    };

    let out = client.tasks_list("@default", &params).unwrap();

    for task in &out.response.items {
        let due = task.due.as_deref().unwrap_or("(no due date)");
        let title = task.title.as_deref().unwrap_or("(no title)");
        println!("{due}: {title}");
    }
}
