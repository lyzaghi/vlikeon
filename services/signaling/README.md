# OmniPlay signaling upstream

Source only. No service or network listener has been started. This process handles signed pairing/control requests and opaque encrypted signaling; browser/native WebRTC handles the media separately. See `docs/STREAMING_PROTOCOL.md` for the exact trust and incomplete product boundaries.

The explicit later build target is `omniplay-signaling-service`. Configure the following process environment through the deployment's secret/configuration mechanism, never through a browser preference or committed private credential:

| Variable | Purpose |
| --- | --- |
| `OMNIPLAY_RELAY_AUDIENCE` | Exact public HTTPS origin without a trailing slash. Must match installed product policy. |
| `OMNIPLAY_RELAY_ORIGINS` | Comma-separated exact HTTPS client origins and/or explicitly trusted `chrome-extension://` component origins. No wildcard. |
| `OMNIPLAY_RELAY_DATA` | Absolute private service-owned data directory with a filesystem quota and backups/retention policy. |
| `OMNIPLAY_RELAY_LISTEN` | Loopback socket only; default `127.0.0.1:8793`. |
| `OMNIPLAY_RELAY_ALLOW_LOOPBACK_WEB` | Optional `1` to allow signed native mobile/authoring clients from exact `http://127.0.0.1:<port>` origins. This expands CORS only; all signatures/grants still apply. |
| `OMNIPLAY_RELAY_TURN_URLS` | Optional comma-separated operator-owned `turn:`/`turns:` URLs, including UDP/TCP variants. |
| `OMNIPLAY_RELAY_TURN_SECRET` | Required with TURN URLs; high-entropy secret shared only with the TURN service, at least 32 bytes. Never embed in a client. |

The service refuses a public HTTP bind. Its TLS ingress must enforce the exact public audience/host, body/concurrency/abuse limits, no secret-body/query logging, TLS policy and private upstream access. TURN requires independent authenticated configuration, certificate/firewall/allocation limits and restrictions against loopback/private service abuse. No ingress, domain, certificate, firewall, TURN listener or operator secret has been provisioned here.

The product's `security/streaming-channel.json` remains disabled and empty. Editing deployment environment alone does not enable the application. Enabling a release requires configured trusted origins, regenerated source inventories/CSP, service deployment, browser/platform credential work and the final full verification. Browser viewing-only capture is an authored path, not completion of native game control or the whole streaming epic.
