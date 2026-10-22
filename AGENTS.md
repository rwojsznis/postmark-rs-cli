# AGENTS.md

You must use ASD-STE100 Simplified Technical English (STE) when it doesn't detract from meaning.

## What this is

`postmark-cli` is an unofficial, **read-only** command-line client for the
[Postmark API](https://postmarkapp.com/developer/api/overview). It is built for
LLM agents and for people who debug email delivery. It supports every GET
endpoint of the Postmark API (44 endpoints), with one subcommand per endpoint.
Single crate (`postmark-rs-cli`), one binary (`postmark-cli`) plus a library
(`postmark_cli`).

Read [`docs/design-notes.md`](docs/design-notes.md) before you change anything
non-trivial. It holds the numbered design decisions. Source comments cite them
by number ("decision 5").

## Sources of truth

- **Postmark's API reference is the specification.** Where postmark-rs differs
  from the docs, the docs win.
- **[postmark-rs](https://github.com/pastjean/postmark-rs) is the transport,
  not the specification** (decision 2). We use its client, its `Endpoint`,
  `Query` and `Client` traits, and its error parsing. We do not use its
  per-endpoint request and response types, because they miss query params and
  endpoints, and their typed responses drop data.
- **The binary is the product.** Integration tests run the real binary.

## Layout

| Path | What |
|---|---|
| `src/main.rs` | Entry point: run, print the error line, choose the exit code |
| `src/lib.rs` | Module list and the panic-family lints |
| `src/client.rs` | `Executor`: one token per request, the timeout, `--verbose`, `QueryError` → `Error`; `--base-url` validation |
| `src/error.rs` | `Error`, its kinds, exit codes, the JSON error line (decision 5) |
| `src/output.rs` | `Format`, json/jsonl rendering, list items for jsonl, warning lines (decisions 3, 4) |
| `src/paginate.rs` | `Plan`: the `--all`/`--limit` loop for offset and cursor pagination, the merged json (decision 9) |
| `src/request.rs` | `GetRequest`: path-segment encoding, the query builder, the `postmark::Endpoint` impl (decisions 1, 2) |
| `src/spec.rs` | `EndpointSpec`, `TokenKind`, `Pagination` |
| `src/cli/mod.rs` | `Cli`, the global flags, the `Command` enum, the one dispatcher (`--dry-run`, the runtime, printing) |
| `src/cli/commands/mod.rs` | The `Leaf` trait: `spec()` + a pure `request()` per endpoint (decision 11) |
| `src/cli/commands/<resource>.rs` | One file per resource: clap args, a `static` spec per leaf, unit tests for `request()` |
| `src/cli/exit.rs` | The exit codes, as a type (decision 5) |
| `src/cli/page.rs` | `PageArgs` (`--count`, `--offset`, validation), `AllArgs` (`--all`, `--limit`) |
| `src/cli/out.rs` | Writes that are safe on a broken pipe, and the print macros |
| `tests/common/mod.rs` | The harness: `cli()`/`run()`, `expect_get` for the mock, the sentinel token, `assert_no_secret` |
| `tests/<resource>.rs` | Integration tests per resource; `tests/errors.rs` for every error path |
| `docs/` | Design notes |

Unit tests live in `#[cfg(test)] mod tests` next to the code; cross-cutting and
CLI-level tests live in `tests/`.

## Workflow — do it in this order

1. **Write the failing test first.** Before any implementation, add a test that
   fails for the reason you expect. Run it and *read the failure* — a test that
   passes immediately, or fails with the wrong message, is not yet a test of the
   thing you meant.
2. **Then write the implementation**, and only enough of it to make that test
   pass.
3. **Check your work.** Run the full suite, not just the new test. If the change
   touches a command, also run the binary and look at the output; `cargo test`
   passing is necessary, not sufficient.
4. **Run the linters last**, and get them clean before you call the work done.

```bash
mise run check      # fmt --check, clippy -D warnings, doc -D warnings, machete, audit, tests
mise run coverage   # cargo llvm-cov summary; new code must be covered
```

These are the same mise tasks that CI runs. `mise.toml` is the only place that
holds the command lines; do not copy them elsewhere.

Keep successful test output brief. We care about failures: if the quiet test
run fails, inspect and report the complete failure output.

Do not report a change as finished until every one of them is green. If
something is left failing or unfinished, say so explicitly rather than narrowing
the scope quietly.

## Coverage

- `mise run coverage` prints the totals.
  `mise exec -- cargo llvm-cov --all-targets --html --open` shows the uncovered
  lines.
- Coverage includes the `postmark-cli` processes that the integration tests
  spawn: cargo-llvm-cov instruments the binary and sets `LLVM_PROFILE_FILE`
  for it. That is why `env_clear()` is forbidden in tests (see Gotchas).
- CI uploads `mise run coverage-lcov` to Codecov. After the first CI run,
  `codecov-coverage-cli --all --sort misses` lists the lines Codecov reports as
  not covered (`--diff main` for one branch).

## Toolchain

mise manages Rust and every dev tool (decision 15).

- Run `mise trust` once in a new checkout, then `mise install`.
- Run cargo through mise's environment: `mise exec -- cargo …`, or a
  `mise run` task.
- One pin per tool. Rust is pinned in `rust-toolchain.toml` only; mise reads
  it. Do not add `rust = "…"` to `mise.toml`: mise exports `RUSTUP_TOOLCHAIN`,
  which overrides the file, so two pins could silently disagree. The dev tools
  are pinned in `mise.toml` `[tools]`.
- Never install a dev tool with `cargo install` or Homebrew. If a tool is
  missing, add it to `mise.toml`, with a backend that downloads a prebuilt
  binary (`aqua:` or `github:`).
- CI installs the same pins with `jdx/mise-action`, and every CI step is a
  mise task. Renovate bumps `rust-toolchain.toml` and `rust-version` together,
  and the `mise.toml` tools through its `mise` manager.

## Where tests go

- Request building, encoding, validation → a unit test in that module.
- A CLI flag, an exit code, an endpoint call → `tests/`, against the real
  binary and a mock Postmark (`--base-url`).
- Each endpoint gets at least one integration test. It asserts the method, the
  path, the query and the token header, that the other token header is absent,
  and that stdout equals the mock body (decision 12).

## Conventions

- **Read-only by construction** (decision 1). The CLI builds one request type,
  `GetRequest`, and its method is always GET. Do not add a method parameter, a
  raw request with a method argument, or any command that changes state.
- **The output is Postmark's JSON, unchanged** (decisions 3 and 4). No
  envelope, no renamed or removed keys, no colors, no tables. The bytes are the
  same on a terminal and on a pipe.
- **Exit codes** (decision 5): **0** success (also when stdout closed early),
  **1** Postmark returned a non-2xx status (`api`), **2** usage or
  configuration error (`config`; clap's own usage errors keep clap's text),
  **3** transport failure (`transport`, `timeout`, `decode`).
- **An error is exactly one JSON line on stderr and nothing on stdout:**
  `{"error":{"kind","message","status","error_code","body","request"}}`.
  `status`/`error_code`/`body` are for `api` only; `request` is there whenever
  a request was built. `--verbose` lines are JSON too: `{"request":{...}}`
  before and `{"response":{"status","ms"}}` after each request.
- **stdout:** `--format json` is pretty (2-space indent, trailing newline);
  `jsonl` is one compact value per line. `serde_json`'s `preserve_order` keeps
  Postmark's key order. Print through `output::print`, never `println!`.
- **Pagination** (decision 9):
  - An offset-paginated leaf flattens `PageArgs` and `AllArgs`, builds its
    request with `self.page.request(&SPEC, &params)?` (validated `count` and
    `offset` first, then its filters), and returns `self.page.paging(&self.all)`
    from `Leaf::paging`. A cursor leaf flattens `AllArgs` only.
  - The spec's `pagination` and `list_key` drive everything else. The pager
    overwrites `count`/`offset`/`paginationKey` per page with
    `GetRequest::set_query`, so the query order stays as the leaf built it.
  - `--limit` pages too, without `--all`. The last page asks only for what is
    left, and never past the `count + offset` window.
  - Stops are warnings, not errors: `pagination_ceiling` at the window,
    `list_key_missing` when a response has no list. Both exit 0.
  - jsonl streams items as pages arrive and stops fetching when stdout
    closes; json prints one merged body (page 1's keys, all items, no
    `PaginationKey`).
- A new error path needs a case in `tests/errors.rs` that runs with the
  sentinel token and calls `assert_no_secret`.
- **Tokens never appear** in stdout, stderr, `--dry-run`, `--verbose` or an
  error message (decision 6). The client gets only the token its endpoint
  needs.
- **Flag names** follow a mechanical rule (decision 8): split the API param
  name into words and join them with hyphens in lowercase (`fromemail` →
  `--from-email`). Enum values keep the exact API spelling.
- Write in-code comments that describe why code or a class does what it does,
  but not what it does. The "what" should be self-evident. Be concise and
  direct. Cite the design decisions: "decision 9".
- Print through the macros in `src/cli/out.rs` (`out!`, `outln!`, `err!`,
  `errln!`), never `println!` or `eprintln!`.

## Gotchas

- **Never `env_clear()` in tests** (decision 12). It removes
  `LLVM_PROFILE_FILE`, and cargo-llvm-cov then loses the coverage of the
  spawned binary. Remove the `POSTMARK_*` variables one by one with
  `env_remove`.
