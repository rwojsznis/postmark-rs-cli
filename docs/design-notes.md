# Design notes

Why `postmark-cli` looks the way it does. Source comments refer to the numbered
decisions here ("decision 5"), so keep the numbering stable.

[Postmark's API reference](https://postmarkapp.com/developer/api/overview) is
the specification. [postmark-rs](https://github.com/pastjean/postmark-rs) is
the transport, not the specification: where the two differ, the docs win.

---

## Decisions

### 1. Read-only by construction

The CLI can build one request type only: `GetRequest`. Its
`postmark::Endpoint::method()` always returns `GET`. No code path takes a
method as a parameter, and there is no generic "raw request" with a method
argument. A catalog test asserts that every command is GET. So an agent can
run any `postmark-cli` command without a confirmation step: no command sends
email or changes state. `README.md` and the top-level `--help` say this.

### 2. postmark-rs is the transport, not the endpoint catalog

We use these parts of `postmark` 2.x (features `reqwest-rustls-tls`):

- `postmark::reqwest::PostmarkClient`: the auth headers, joining the base URL,
  and reqwest with rustls (no OpenSSL, so musl builds stay static).
- The `Endpoint`, `Query` and `Client` traits.
- `QueryError::Api { status, error_code, message, body }`: it parses
  Postmark's error body.

We do **not** use postmark-rs's per-endpoint request and response structs:

- **Query params are missing.** Outbound search has no `fromemail`, `status`,
  `subject`, `fromdate`, `todate` or `metadata_*`. Inbound search has no
  `recipient`, `fromemail`, `tag`, `status` or dates, and it sends an
  undocumented `to`. Opens and clicks have no client, OS, platform or geo
  filters.
- **Two GET endpoints are missing:** `GET /email/bulk` and
  `GET /webhooks/{Id}/statistics`.
- **Typed responses lose data and fail on drift.** serde drops unknown fields.
  Non-optional fields (`isize`, `String`) fail to decode when Postmark leaves
  them out, and Postmark leaves out empty properties (see the Bulk API docs).

So we have our own `GetRequest`, which implements `postmark::Endpoint` with
`type Response = serde_json::Value`. postmark-rs's request types are still
useful as a **test oracle**: `tests/postmark_rs_parity.rs` asserts that, for
the same inputs, our path and query match theirs wherever postmark-rs has the
endpoint.

`PostmarkClient` does not let us configure its inner `reqwest::Client`: no
timeout, no user agent, no proxy. We wrap each request in
`tokio::time::timeout` (decision 7). If we later need a user agent or proxy
support, implement `postmark::Client` ourselves (~40 lines; see postmark-rs
`src/reqwest.rs`). Nothing needs that yet.

### 3. The output is Postmark's JSON, unchanged

On success, stdout is the response body as a `serde_json::Value`. Turn on
`serde_json`'s `preserve_order` feature so the key order matches Postmark's.
There is no envelope, and no keys are renamed or removed. Agents know
Postmark's documented shapes, and the output matches them. The one exception
is `--all` (decision 9), which merges pages into one object of the same shape.

### 4. Two output formats, no TTY tricks

`--format json` (default) prints pretty JSON with 2-space indent and a
trailing newline. `--format jsonl` prints one compact JSON value per line:

- For a list endpoint, one line per item of the list key (see the
  `list_key` that `postmark-cli commands` reports for it, decision 10).
- For any other endpoint, the whole body on one line.

There are no colors, no tables, no spinners and no prompts. The bytes are the
same whether stdout is a terminal or a pipe.

### 5. Errors and exit codes

| Exit | Meaning | `error.kind` |
|---|---|---|
| 0 | Success. Also when stdout closed early (broken pipe). | — |
| 1 | Postmark answered with a non-2xx status | `api` |
| 2 | Usage or configuration error: clap usage errors, a missing token, a bad base URL, `--count` over the endpoint's limit, a bad date | `config` (clap usage errors keep clap's own text) |
| 3 | Transport failure: connect, TLS, timeout, or a body that is not JSON | `transport`, `timeout`, `decode` |

An error prints exactly one JSON line on stderr and nothing on stdout:

```json
{"error":{"kind":"api","message":"Postmark returned 422: Message not found","status":422,"error_code":701,"body":{"ErrorCode":701,"Message":"..."},"request":{"method":"GET","url":"https://api.postmarkapp.com/messages/outbound/abc/details"}}}
```

- `status`, `error_code` and `body` are present for `api` only. `body` is
  the parsed JSON when the body is JSON. Otherwise it is a string, cut to
  4 KiB.
- `request` is present whenever a request was built.
- A token never appears in any output (decision 6).
- Warnings that do not fail the run (for example the pagination ceiling) are
  one JSON line on stderr: `{"warning":{"kind":"...","message":"..."}}`.

A closed stdout pipe is not an error. Port i18n-tasks-rs `src/cli/out.rs`, and
make `write_out` report a closed pipe so that `--all` stops fetching.

### 6. Tokens

- `--server-token <TOKEN>` / `POSTMARK_SERVER_TOKEN`
- `--account-token <TOKEN>` / `POSTMARK_ACCOUNT_TOKEN`

Both use clap `env` with `hide_env_values = true`, so `--help` never prints a
token. Each command's spec declares the token kind it needs (from the docs).
The client gets **only** that token, so the other token is never sent. A
missing token is a `config` error that names both the env var and the flag.

Tokens never appear in stdout, stderr, `--dry-run`, `--verbose`, or error
messages. A test runs every error path with a sentinel token string and
asserts that the string does not appear in the output.

### 7. Global flags

All of these use `global = true`, so they work before or after the subcommand.

| Flag | Env | Default | What |
|---|---|---|---|
| `--server-token` | `POSTMARK_SERVER_TOKEN` | — | decision 6 |
| `--account-token` | `POSTMARK_ACCOUNT_TOKEN` | — | decision 6 |
| `--base-url` | `POSTMARK_BASE_URL` | `https://api.postmarkapp.com/` | Tests point it at a mock server. Must be http or https. |
| `--timeout <SECONDS>` | — | `30` | Per request, through `tokio::time::timeout` |
| `--format <json\|jsonl>` | — | `json` | decision 4 |
| `--dry-run` | — | off | Prints `{"method":"GET","url":"…","token":"server"}` on stdout and exits 0. No network, and no token needed. With `--all`, it prints the first page's request. |
| `-v`, `--verbose` | — | off | One stderr line before each request (`GET <url>`) and one after it (`<status> in <ms> ms`). The lines are JSON, like the warnings. |

### 8. Command tree

The shape is `postmark-cli <resource> <verb> [PATH_PARAMS] [--query-flags]`.
One leaf command calls one endpoint. Path params are positional. Query params
are flags.

**Flag names follow a mechanical rule:** split the API param name into words,
then join them with hyphens in lowercase. So `fromemail` becomes
`--from-email`, `messagestream` becomes `--message-stream`, `emailFilter`
becomes `--email-filter`, `messageID` becomes `--message-id`, `client_name`
becomes `--client-name`, and `IncludeArchivedStreams` becomes
`--include-archived-streams`. Each flag's help names the API param that it
sends.

- **Enums:** clap `ValueEnum`. The possible values are the **exact API
  spelling** (`HardBounce`, `Transactional`), accepted with
  `ignore_case = true`, and the exact spelling is sent.
- **Tri-state booleans** (`inactive`): use `Option<bool>` with an explicit
  value (`--inactive true|false`). Leaving it out means "both".
- **Plain opt-in booleans** (`IncludeArchivedStreams`): use a bare flag that
  sends `true`.
- **Dates:** a value parser that accepts `YYYY-MM-DD`, plus
  `YYYY-MM-DDTHH:MM:SS` where the endpoint accepts a time. A bad date is
  exit 2 before any request. The user's string is sent unchanged. The help
  says the time zone (Eastern Time, per the docs). The `time` crate is already
  a dependency of postmark-rs, so use it for parsing.
- **`metadata_<key>`** (outbound search only, one key per request):
  `--metadata KEY=VALUE` sends `metadata_KEY=VALUE`. The flag is not
  repeatable.
- **Query param spelling:** send the exact spelling from the docs' parameter
  table. The one exception is `count`/`offset`, which we always send in
  lowercase (the docs' curl examples and postmark-rs do this, including for
  `/templates`). The live tests confirm it.

| Command | Endpoint | Token |
|---|---|---|
| `server get` | `/server` | server |
| `servers list` | `/servers` | account |
| `servers get <SERVER_ID>` | `/servers/{serverid}` | account |
| `outbound search` | `/messages/outbound` | server |
| `outbound get <MESSAGE_ID>` | `/messages/outbound/{messageid}/details` | server |
| `outbound dump <MESSAGE_ID>` | `/messages/outbound/{messageid}/dump` | server |
| `inbound search` | `/messages/inbound` | server |
| `inbound get <MESSAGE_ID>` | `/messages/inbound/{messageid}/details` | server |
| `opens list` | `/messages/outbound/opens` | server |
| `opens for-message <MESSAGE_ID>` | `/messages/outbound/opens/{messageid}` | server |
| `clicks list` | `/messages/outbound/clicks` | server |
| `clicks for-message <MESSAGE_ID>` | `/messages/outbound/clicks/{messageid}` | server |
| `bounces delivery-stats` | `/deliverystats` | server |
| `bounces list` | `/bounces` | server |
| `bounces get <BOUNCE_ID>` | `/bounces/{bounceid}` | server |
| `bounces dump <BOUNCE_ID>` | `/bounces/{bounceid}/dump` | server |
| `bulk list` | `/email/bulk` | server |
| `bulk get <BULK_REQUEST_ID>` | `/email/bulk/{bulk-request-id}` | server |
| `templates list` | `/templates` | server |
| `templates get <ID_OR_ALIAS>` | `/templates/{templateIdOrAlias}` | server |
| `streams list` | `/message-streams` | server |
| `streams get <STREAM_ID>` | `/message-streams/{stream_ID}` | server |
| `suppressions dump <STREAM_ID>` | `/message-streams/{stream_id}/suppressions/dump` | server |
| `webhooks list` | `/webhooks` | server |
| `webhooks get <WEBHOOK_ID>` | `/webhooks/{Id}` | server |
| `webhooks stats <WEBHOOK_ID>` | `/webhooks/{Id}/statistics` | server |
| `inbound-rules list` | `/triggers/inboundrules` | server |
| `stats overview` | `/stats/outbound` | server |
| `stats sends` | `/stats/outbound/sends` | server |
| `stats bounces` | `/stats/outbound/bounces` | server |
| `stats spam` | `/stats/outbound/spam` | server |
| `stats tracked` | `/stats/outbound/tracked` | server |
| `stats opens` | `/stats/outbound/opens` | server |
| `stats opens-platforms` | `/stats/outbound/opens/platforms` | server |
| `stats opens-email-clients` | `/stats/outbound/opens/emailclients` | server |
| `stats clicks` | `/stats/outbound/clicks` | server |
| `stats clicks-browser-families` | `/stats/outbound/clicks/browserfamilies` | server |
| `stats clicks-platforms` | `/stats/outbound/clicks/platforms` | server |
| `stats clicks-location` | `/stats/outbound/clicks/location` | server |
| `domains list` | `/domains` | account |
| `domains get <DOMAIN_ID>` | `/domains/{domainid}` | account |
| `senders list` | `/senders` | account |
| `senders get <SIGNATURE_ID>` | `/senders/{signatureid}` | account |
| `data-removals get <ID>` | `/data-removals/{id}` | account |
| `commands` | — (no network; decision 10) | — |

That is 44 endpoint commands plus `commands`. The path templates use the
docs' own placeholder names. A placeholder is any `{...}`.

### 9. Pagination

Each spec declares its pagination style:

- **`None`**: no pagination.
- **`OffsetCount { max_count, max_window }`**: `max_window` is
  `Some(10_000)` where the docs state a count + offset ceiling.
- **`Cursor`**: `GET /email/bulk` only. It takes `count` and `paginationKey`,
  and the response's `PaginationKey` is missing on the last page.

Commands with `OffsetCount` flatten a shared `PageArgs`:

- `--count <N>`: default **50**, from 1 to `max_count`.
- `--offset <N>`: default 0. `count + offset` must not exceed `max_window`.
- A breach is a `config` error, exit 2, before any request is made.

Every paginated command also gets:

- `--all`: fetch pages one after another. The page size is `max_count`
  unless `--count` is given. Stop at `TotalCount`, at an empty or short page,
  or at the `max_window` ceiling. At the ceiling, print a
  `pagination_ceiling` warning that tells the user to narrow the dates.
- `--limit <N>`: stop after N items in total.

The output of `--all` depends on the format:

- `json`: one merged object of the original shape, `{"TotalCount": n,
  "<ListKey>": [all items]}`. For cursor endpoints, there is no
  `PaginationKey` in the merged output.
- `jsonl`: items are printed as each page arrives. If stdout closes, stop
  fetching.

Pages are fetched one at a time, never in parallel, so the output is
deterministic and the client stays polite to the API.

### 10. Agents can discover every command

`postmark-cli commands` prints the catalog as JSON. It needs no network and
no token. Each entry has these fields:

- `command`: for example `"outbound search"`
- `summary`
- `method`: always `"GET"`
- `path`: the template
- `token`: `"server"` or `"account"`
- `pagination`: `"none"`, `"offset"` or `"cursor"`, with limits
- `list_key`
- `args`: `[{name, flag, positional, required, help, possible_values,
  default}]`

`args` comes from **clap's own `Command` introspection**, so it cannot drift
from the parser. Each leaf's `--help` ends with `Endpoint: GET <path>`,
`Token: <kind>` and one example. That text is generated from the same spec,
not written twice. The top-level `--help` lists the env vars, the exit codes,
the read-only guarantee, and a pointer to `commands`.

### 11. One place per command

Each resource has one module, `src/cli/commands/<resource>.rs`. Each leaf in
it has:

- a clap `Args` struct
- a `static SPEC: EndpointSpec` (path template, token, summary, pagination,
  list key)
- a pure `fn request(&self) -> Result<GetRequest, ConfigError>`, unit-tested
  without a network

One registry maps a leaf's command path (for example `["outbound",
"search"]`) to its spec. Execution, pagination, output and errors are written
once, in the shared code, and are not repeated per command.

### 12. Tests

- **Unit tests** go next to the code: request building, encoding, validation.
- **Integration tests** go in `tests/`. They run the real binary
  (`env!("CARGO_BIN_EXE_postmark-cli")`) against an `httptest` mock server
  through `--base-url`. Each endpoint gets at least one test that asserts:
  - the method, the path, the query and the token header
  - that the other token header is **absent**
  - that the parsed stdout equals the mock body
- **Environment:** remove every `POSTMARK_*` var from the child process
  (`env_remove`). **Never use `env_clear()`.** It removes
  `LLVM_PROFILE_FILE`, and then cargo-llvm-cov loses the coverage of the
  spawned binary.
- **`--dry-run`** is the cheap way to test URL building through clap.
- **Parity** (`tests/postmark_rs_parity.rs`): compare our path and query
  with postmark-rs's request types, where they exist. Compare only the params
  both sides support.
- **Completeness:** `tests/fixtures/get-endpoints.txt` holds the 44 path
  templates. A test asserts that the catalog plus `NOT_YET_IMPLEMENTED` equals
  the fixture, and that the two do not overlap. `NOT_YET_IMPLEMENTED` is
  empty once every endpoint has its command.
- **Live tests** (`tests/live.rs`): every test is `#[ignore]`. They read
  `POSTMARK_LIVE_SERVER_TOKEN` and `POSTMARK_LIVE_ACCOUNT_TOKEN` and are never
  run in CI. They are safe because of decision 1.

### 13. Tooling parity with i18n-tasks-rs

Copy these from i18n-tasks-rs:

- the `rust-toolchain.toml` pin and the `rust-version` = toolchain rule
- the lint tables, and the panic-family lints in `lib.rs`/`main.rs` with the
  `clippy.toml` test exemptions
- the CI jobs: fmt, clippy `-D warnings`, doc `-D warnings`, machete, audit,
  a Linux + macOS test matrix, and coverage through cargo-llvm-cov to Codecov
- Renovate, with its workflow, digest pinning and automerge
- the tag-triggered release workflow for four targets

The one deliberate difference is how the tools get installed (decision 15).

### 14. Async runtime

postmark-rs is async. `main` builds one tokio **current-thread** runtime
(features `rt` and `time` only), runs the command on it, and maps the result
to an exit code. There is no concurrency anywhere (decision 9).

### 15. mise manages the toolchain and every dev tool

The rule is **one pin per tool, read the same way locally and in CI.**

- **Rust: always the latest stable, pinned exactly.** The pin is in
  `rust-toolchain.toml`: `channel = "1.99.1"`, the latest stable when the
  project started (released 2026-10-13). Its components are `rustfmt`, `clippy` and
  `llvm-tools` (local coverage is part of the definition of done). Its
  profile is `minimal`.
  - The project's `mise.toml` turns on
    `[settings] idiomatic_version_file_enable_tools = ["rust"]`. With that,
    mise installs and selects the toolchain *from* `rust-toolchain.toml`.
  - The pin is not repeated as `rust = "…"` in `mise.toml`. mise exports
    `RUSTUP_TOOLCHAIN`, which overrides `rust-toolchain.toml`, so two pins
    could silently disagree.
  - rustup, IDEs and the release workflow read the same file without mise.
  - Renovate already bumps `rust-toolchain.toml` in i18n-tasks-rs (its "Rust
    toolchain" group). Together with the `rust-version` custom manager, that
    keeps us on the latest stable through daily PRs. Do not use
    `channel = "stable"`: an unpinned channel makes builds irreproducible.
- **Dev tools:** `mise.toml` `[tools]` pins exact versions. Prefer
  backends with prebuilt binaries:

  | Tool | Backend | Version (2026-10-22) |
  |---|---|---|
  | cargo-llvm-cov | `aqua:taiki-e/cargo-llvm-cov` | 0.9.1 |
  | cargo-machete | `github:bnjbvr/cargo-machete` | 0.9.2 |
  | cargo-audit | `cargo:cargo-audit` | 0.22.2 |
  | cargo-binstall | `aqua:cargo-bins/cargo-binstall` | 1.26.0 |
  | actionlint | `aqua:rhysd/actionlint` | 1.7.12 |

  cargo-binstall is there so that the `cargo:` backend downloads a binary
  instead of compiling one.
- **mise tasks are the single list of check commands:**
  - `fmt`, `clippy`, `doc`, `machete`, `audit`, and `lint` (all five)
  - `test`
  - `coverage`
  - `actionlint`
  - `check`, which runs lint and test

  `AGENTS.md` and the CI workflow call
  these tasks. None of them repeats the cargo command lines.
- **CI** installs everything with `jdx/mise-action`, pinned by digest. So CI
  runs the same versions as a laptop. This replaces the reference's
  `rustup show active-toolchain` + `cargo install --version` steps, and
  their `# renovate:` regex manager.
- **Renovate** keeps the tools current through its built-in `mise` manager.
  For any tool it does not extract, add a custom regex manager.

## Module layout (target)

| Path | What |
|---|---|
| `src/main.rs` | Entry point: run, print the error, choose the exit code |
| `src/lib.rs` | Module list and the panic-family lints |
| `src/request.rs` | `GetRequest`, path-segment encoding, query builder, the `postmark::Endpoint` impl (decisions 1, 2) |
| `src/spec.rs` | `EndpointSpec`, `TokenKind`, `Pagination` |
| `src/client.rs` | Build a `PostmarkClient` for one token kind, run with a timeout, map `QueryError` to `Error` |
| `src/error.rs` | `Error`, its kinds, exit codes, JSON rendering (decision 5) |
| `src/output.rs` | json/jsonl rendering, extracting list items (decisions 3, 4) |
| `src/paginate.rs` | The `--all` loop for offset and cursor pagination (decision 9) |
| `src/cli/mod.rs` | `Cli`, the global flags, the `Command` enum, dispatch |
| `src/cli/out.rs` | Writes that are safe on a broken pipe, and the print macros (ported) |
| `src/cli/page.rs` | `PageArgs`, `AllArgs` |
| `src/cli/args.rs` | Shared value parsers: dates, `KEY=VALUE` |
| `src/cli/catalog.rs` | The registry and the `commands` subcommand (decision 10) |
| `src/cli/commands/<resource>.rs` | One file per resource group |
| `tests/common/mod.rs` | The binary runner, the mock-server helpers, the sentinel token |
| `tests/<resource>.rs` | Integration tests per resource |
| `tests/{catalog,errors,postmark_rs_parity,broken_pipe,live}.rs` | Cross-cutting tests |
| `docs/design-notes.md` | These decisions |
| `docs/endpoints.md` | The command → endpoint matrix, checked against the registry by a test |
