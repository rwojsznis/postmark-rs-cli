# Endpoints

Every command, the endpoint it calls, and the token it sends. All are GET
(decision 1). `tests/catalog.rs` generates this table from the registry;
after a change, run `UPDATE_DOCS=1 cargo test --test catalog docs_endpoints`
and review the diff.

| Command | Method | Path | Token | Pagination | List key | postmark-rs | Notes |
|---|---|---|---|---|---|---|---|
| `server get` | GET | `/server` | server | none | — | `GetCurrentServerRequest` | The server that owns the token. |
| `servers list` | GET | `/servers` | account | offset, max 500 | `Servers` | `ListServersRequest` | postmark-rs sends no `name`. |
| `servers get` | GET | `/servers/{serverid}` | account | none | — | `GetServerRequest` |  |
