# postmark-cli

A command-line client for the [Postmark API](https://postmarkapp.com/developer/api/overview),
built for LLM agents and for people who debug email delivery. It supports every
GET endpoint of the Postmark API, with one subcommand per endpoint, and prints
Postmark's JSON unchanged.

> [!NOTE]
> **Unofficial.** This project is not affiliated with Postmark or ActiveCampaign.

**Read-only.** `postmark-cli` can only send GET requests. No command sends
email or changes state, so an agent can run any command without asking for
confirmation.

Status: work in progress.

## License

MIT. See [LICENSE.txt](LICENSE.txt).
