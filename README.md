# postmark-cli
[![codecov](https://codecov.io/gh/rwojsznis/postmark-rs-cli/graph/badge.svg)](https://codecov.io/gh/rwojsznis/postmark-rs-cli)
[![CI](https://github.com/rwojsznis/postmark-rs-cli/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/rwojsznis/postmark-rs-cli/actions/workflows/ci.yml?query=branch%3Amain)

A command-line client for the [Postmark API](https://postmarkapp.com/developer/api/overview),
built for LLM agents and for people who debug email delivery. It covers every
GET endpoint of the Postmark API (44), one subcommand per endpoint, and prints
Postmark's JSON unchanged.

> [!NOTE]
> **Unofficial.** This project is not affiliated with Postmark or ActiveCampaign.

**Read-only.** `postmark-cli` sends GET requests only. No command sends email
or changes anything, so an agent can run any command without asking first.

## Install

Download a binary for macOS or Linux from the
[latest release](https://github.com/rwojsznis/postmark-rs-cli/releases/latest),
unpack it, and put `postmark-cli` on your `PATH`. Or build it:

```bash
cargo install --git https://github.com/rwojsznis/postmark-rs-cli --locked
```

## Auth

Postmark has two kinds of token, and each command needs one of them:

| Token | Env var | Flag | Commands |
|---|---|---|---|
| Server | `POSTMARK_SERVER_TOKEN` | `--server-token` | everything about one server: messages, bounces, opens, clicks, templates, streams, suppressions, webhooks, inbound rules, bulk, stats, `server get` |
| Account | `POSTMARK_ACCOUNT_TOKEN` | `--account-token` | `servers`, `domains`, `senders`, `data-removals` |

A command sends only the token it needs, so the other one never leaves your
machine. Tokens never appear in any output: not in results, errors,
`--verbose` or `--dry-run`, and `--help` shows the variable names, not their
values.

## Quick start: why did this email not arrive?

```bash
export POSTMARK_SERVER_TOKEN=...

# Find the message. Dates are Eastern Time.
postmark-cli outbound search --recipient jane@example.com --from-date 2026-10-20

# Its details, including every event (delivered, bounced, opened...).
postmark-cli outbound get 07311c54-0687-4ab9-b034-b54b5bad88ba

# Did the address bounce, and is it now inactive?
postmark-cli bounces list --email-filter jane@example.com
postmark-cli bounces get 692560173

# Is it on the stream's suppression list?
postmark-cli suppressions dump outbound --email-address jane@example.com

# Did she open it, and with which client?
postmark-cli opens for-message 07311c54-0687-4ab9-b034-b54b5bad88ba

# Inbound: blocked and failed messages are hidden unless you ask for them.
postmark-cli inbound search --status blocked --recipient support@example.com

# The big picture.
postmark-cli stats overview --from-date 2026-10-01
```

Every command has `--help`, which ends with the endpoint it calls, the token
it needs and an example. [`docs/endpoints.md`](docs/endpoints.md) lists all 44
commands.

## Output and exit codes

- **stdout** is Postmark's JSON, unchanged: the same keys in the same order.
  `--format json` (the default) prints it pretty; `--format jsonl` prints one
  compact value per line, and for a list endpoint, one line per item. There
  are no colors, tables or prompts: the bytes are the same on a terminal and
  in a pipe.
- **stderr** gets one JSON line per error or warning:

  ```json
  {"error":{"kind":"api","message":"Postmark returned 401: ...","hint":"Check POSTMARK_SERVER_TOKEN ...","status":401,"error_code":10,"body":{...},"request":{"method":"GET","url":"https://api.postmarkapp.com/server"}}}
  ```

| Exit | Meaning | `error.kind` |
|---|---|---|
| 0 | Success, also when stdout closes early (`\| head`) | — |
| 1 | Postmark answered with an error status | `api` |
| 2 | Usage or configuration error; nothing was sent | `config` (clap's own text for bad flags) |
| 3 | No usable answer: connection, TLS, timeout, or a body that is not JSON | `transport`, `timeout`, `decode` |

## For AI agents

- **Discover the commands:** `postmark-cli commands` prints a JSON catalog of
  every command: its endpoint, token, pagination, list key, example and
  flags (read from the parser itself, so it is always current). It needs no
  token and no network.
- **Read-only by construction:** the tool can only build GET requests. Running
  any command is safe.
- **Use `--format jsonl`** for list endpoints: one item per line, easy to
  filter with `jq` or to stream.
- **Check before you send:** `--dry-run` prints the exact request URL and the
  token kind, without a token and without the network.
- **Errors are JSON** on stderr, with a `hint` for the Postmark error codes an
  agent can act on (a bad token, Bulk API not enabled, a date too old, a
  count too high).
- **Pagination:** `--count` and `--offset` get one page. `--all` fetches every
  page, one at a time, and `--limit N` stops after N items. Search endpoints
  stop at 10,000 items with a `pagination_ceiling` warning; narrow the dates
  to get the rest.
- **`--verbose`** logs each request and its status and time on stderr, as JSON
  lines.

## License

MIT. See [LICENSE.txt](LICENSE.txt).
