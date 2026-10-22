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
| `outbound search` | GET | `/messages/outbound` | server | offset, max 500, window 10000 | `Messages` | `OutboundSearchRequest` | Window: count + offset ≤ 10,000. |
| `outbound get` | GET | `/messages/outbound/{messageid}/details` | server | none | — | `OutboundDetailsRequest` |  |
| `outbound dump` | GET | `/messages/outbound/{messageid}/dump` | server | none | — | `OutboundDumpRequest` |  |
| `inbound search` | GET | `/messages/inbound` | server | offset, max 500, window 10000 | `InboundMessages` | `InboundSearchRequest` | Window: count + offset ≤ 10,000. Default `status=processed`. |
| `inbound get` | GET | `/messages/inbound/{messageid}/details` | server | none | — | `InboundDetailsRequest` |  |
| `opens list` | GET | `/messages/outbound/opens` | server | offset, max 500, window 10000 | `Opens` | `MessageOpensRequest` | Window: count + offset ≤ 10,000. |
| `opens for-message` | GET | `/messages/outbound/opens/{messageid}` | server | offset, max 500 | `Opens` | `SingleMessageOpensRequest` |  |
| `clicks list` | GET | `/messages/outbound/clicks` | server | offset, max 500 | `Clicks` | `MessageClicksRequest` |  |
| `clicks for-message` | GET | `/messages/outbound/clicks/{messageid}` | server | offset, max 500 | `Clicks` | `SingleMessageClicksRequest` |  |
| `bounces delivery-stats` | GET | `/deliverystats` | server | none | — | `GetDeliveryStatsRequest` |  |
| `bounces list` | GET | `/bounces` | server | offset, max 500, window 10000 | `Bounces` | `ListBouncesWithFiltersRequest` | Window: count + offset ≤ 10,000. |
| `bounces get` | GET | `/bounces/{bounceid}` | server | none | — | `GetBounceRequest` |  |
| `bounces dump` | GET | `/bounces/{bounceid}/dump` | server | none | — | `GetBounceDumpRequest` | Dumps are kept for 30 days. |
| `templates list` | GET | `/templates` | server | offset, max 500 | `Templates` | `ListTemplatesRequest` |  |
| `templates get` | GET | `/templates/{templateIdOrAlias}` | server | none | — | `GetTemplateRequest` | An ID or an alias. |
| `streams list` | GET | `/message-streams` | server | none | `MessageStreams` | `ListMessageStreamsRequest` | Not paginated. |
| `streams get` | GET | `/message-streams/{stream_ID}` | server | none | — | `GetMessageStreamRequest` |  |
| `suppressions dump` | GET | `/message-streams/{stream_id}/suppressions/dump` | server | none | `Suppressions` | `GetSuppressionRequest` | Not paginated. |
