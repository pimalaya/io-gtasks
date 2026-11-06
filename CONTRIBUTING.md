# Contributing guide

Thank you for investing your time in contributing to I/O Google Tasks.

Whether you are a human or an AI agent, read these in order before touching the code:

1. the [Pimalaya README](https://github.com/pimalaya) for what the project is and how its repositories stack;
2. the [Pimalaya CONTRIBUTING](https://github.com/pimalaya/.github/blob/master/CONTRIBUTING.md) guide, which chains to the shared architecture and guidelines;
3. the inline header documentation, starting with src/lib.rs: it is the architecture document of this crate;
4. the [cairn/](./cairn) folder for the development history and living plans (the Cairn convention: spec/, changes/, log/).

Everything below documents only what differs from the Pimalaya standards.

## Cairn

This repository follows [Cairn](https://github.com/pimalaya/cairn): a living spec, reviewable change proposals and a dated log, kept next to the code. Non-trivial work starts with a change folder under cairn/changes, and nothing behavioural is done until its delta is folded into cairn/spec and an entry is appended to cairn/log. The activation stanza is [AGENTS.md](./AGENTS.md).

## Feature matrix

io-gtasks follows the standard layered split (I/O-free coroutines, then the std client behind the `client` feature), plus the iCalendar projection and a vendored switch compiling the TLS dependencies from source:

```sh
cargo build --no-default-features                        # coroutines only, no std leak
cargo build --no-default-features --features ical        # coroutines and the VTODO projection
cargo build --no-default-features --features client      # light client, no TLS deps
cargo build                                              # full client (rustls-ring by default)
cargo build --no-default-features --features rustls-aws  # full client, aws-lc-rs crypto
cargo build --no-default-features --features native-tls  # full client, platform TLS
cargo build --features vendored                          # vendored TLS dependencies
```

## Tests

The default suite is fully offline: every coroutine is driven against an in-memory stream replaying a canned HTTP response, so no network access nor OAuth token is required.

```sh
cargo test --all-features
```

The suites under tests/ mirror the source tree, one file per resource plus the cross-cutting ones:

| Suite                   | What it covers                                                                                      |
|-------------------------|------------------------------------------------------------------------------------------------------|
| tests/common/mod.rs     | The scripted-coroutine harness: canned responses in, request bytes and terminal value out             |
| tests/send.rs           | The transport: the authorized request, the path encoding, the etag guard, the error envelope, the redirect refusal, the keep-alive flag |
| tests/query.rs          | The query serializer: what reaches the URL and what deliberately does not                             |
| tests/tasklists.rs      | Every task list method's request line, query, body and parsed response                                |
| tests/tasks.rs          | Every task method's request line, query, body and parsed response, and the wire spelling of the enums |
| tests/client.rs         | The std client over a scripted stream, including its error paths                                      |
| tests/google.rs         | The live tests, ignored by default                                                                    |

The VTODO projection is covered by the unit tests at the bottom of src/v1/rest/tasks/ical.rs.

A test asserts on the request as written on the wire, since the query keys are camelCase and a silent rename is exactly the kind of regression the offline suite exists to catch.

tests/google.rs holds ignored end-to-end tests against the live Tasks API, needing a TLS feature and either a `GTASKS_ACCESS_TOKEN` or a service account key. `account` only reads; `tasks` walks the whole surface on two throwaway task lists, and `ical` writes a task from a VTODO and patches it from an edited one; both delete their lists however the run ends. Each runs once as the service account and once as the Workspace user it acts for through domain-wide delegation:

```sh
GTASKS_ACCESS_TOKEN=<token> cargo test --features ical --test google -- --ignored --test-threads 1
```

The tests do no OAuth of their own, so any grant that yields a bearer token with the https://www.googleapis.com/auth/tasks scope works.

## Minting a token without a human

A token from the authorization-code flow acts as you and expires within the hour, which rules out an unattended run. A service account instead signs its own assertion and trades it for a token whenever one is needed, the JWT-bearer grant of RFC 7523 section 2.1, and the tests do that themselves through io-oauth when handed a key rather than a token:

```sh
GTASKS_SERVICE_ACCOUNT_KEY_FILE=key.json cargo test --features ical --test google -- --ignored --test-threads 1
```

`GTASKS_SERVICE_ACCOUNT_KEY` takes the key inline instead of by path, which is what CI passes straight out of its secret. `GTASKS_ACCESS_TOKEN` still short-circuits both when you already hold a token. `GTASKS_SERVICE_ACCOUNT_SUBJECT` names the Workspace user of the delegated runs, `google@pimalaya.org` by default.

A service account is a Tasks principal in its own right: it owns a default list and the lists it creates, so the non-delegated runs need no role and no Workspace domain, only the Tasks API enabled on the project (https://console.cloud.google.com/apis/library/tasks.googleapis.com), or every call answers 403. The delegated runs also need the tasks scope granted to the service account's client id under the Workspace domain-wide delegation settings.

Minting in process rather than in a shell is deliberate. The token is derived from the secret, so GitHub cannot know to redact it; keeping it inside the test process means it is never written to a file, never passed between steps, and never lands anywhere a log could pick it up. io-http redacts `authorization` in its `Debug` implementation, so it stays out of the trace log too.

## Coverage

cargo-tarpaulin ships in the devshell, so the number CI reports can be reproduced locally:

```sh
cargo tarpaulin --engine llvm --all-features --out Stdout
```

The offline suites cover about 96% of the lines. What is left out is `GtasksClientStd::connect`, which opens a real TCP and TLS connection and therefore cannot run offline, a few conveniences of `GtasksField`, plus a handful of lines inside generic functions that the llvm instrumentation attributes to no test even though the suites demonstrably execute them. Do not reshape the code to chase those.

## CI

Three jobs run out of .github/workflows/tests.yml. The shared Pimalaya `tests` job builds and runs the offline suites on every push, and a `coverage` job reports tarpaulin's line coverage to Codecov.

The `google-tests` job runs the live tests on every push, from the service account key stored as the `GOOGLE_SERVICE_ACCOUNT_KEY` organization secret and passed to the tests as `GTASKS_SERVICE_ACCOUNT_KEY`. The tests trade that key for a fresh token themselves, so no token is stored or passed between steps. The job is guarded on the repository name, since a fork holds no secret and would fail on every contributor's push.

## Checking the API surface against the reference

The Tasks API publishes a machine-readable discovery document, which is the source of truth for the resources, methods, query parameters and schemas the crate mirrors:

```sh
curl 'https://tasks.googleapis.com/$discovery/rest?version=v1'
```

Any addition to the crate should be checked against it rather than against memory of the HTML reference.
