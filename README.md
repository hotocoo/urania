# urania

No server. No port. No tray. No dashboard. Zero-permission probes. One static binary.

urania is a headless network evidence logger for servers, Docker, CI, and SSH
sessions. It continuously probes DNS, TCP, and HTTP (ICMP optional), records
every result to a local SQLite database, detects outage windows with
per-layer cause attribution, and renders a self-contained HTML report — the
artifact you attach to an ISP or IT ticket.

> netband tells you the network is bad right now.
> urania proves it was bad last Tuesday.

Named for Urania, the Muse of astronomy — the one who watches the heavens.
urania watches your network's sky around the clock and records what it sees,
so you can prove it later.

## Why

- **Zero permissions.** Default probes use plain TCP/HTTP/DNS — no root, no
  raw sockets, no firewall prompts.
- **No local server.** No open port, no browser, nothing to lock down.
- **One static binary.** Rust; no runtime dependencies; musl or darwin.
- **Evidence integrity.** Multi-target defaults, per-probe resolver provenance,
  wall clock + monotonic delta per probe, optional SHA-256 manifest.

## Install

### From source

```
cargo build --release
# binary: target/release/urania
```

### Prebuilt binaries

GitHub Releases: 4 static binaries (x86_64/aarch64 × linux-musl/darwin) +
SHA256SUMS.

### systemd (Linux)

```
sudo install -m 755 target/release/urania /usr/bin/urania
sudo cp systemd/urania.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now urania
```

### Docker

```
docker build -t urania .
docker run -d --name urania \
  -v urania-data:/root/.local/share/urania urania
```

The image is a scratch container with one fully static musl binary (built
for the host architecture; for other arches use the GitHub Releases
binaries). The database lives in the mounted volume; `docker exec urania
report --since 24h` writes the report into the container's working directory.

## Usage

```
urania start            # start the background daemon
urania stop             # stop (SIGTERM, graceful, up to 10s)
urania status           # daemon, db, per-target last probe + 1h availability, open outages
urania status --check   # scripts: print state, exit 0 = running, 1 = not running
urania once             # one probe round, table output, exit 0/1
urania once --type tcp  # only TCP targets
urania report --since 24h --to report.html
urania report --since 30d --to report.html --csv report.csv --manifest
urania verify report.html   # check the report against report.html.sha256
urania config           # show the effective config (file + resolved paths + targets)
urania selftest         # round-trip check against known-good targets
```

Exit codes: `once`/`selftest` 0 = all ok, 1 = any failure; `start` 2 =
already running; `status --check` 1 = not running; `report` 3 = no data in
period; `verify` 1 = hash mismatch or missing file, 2 = unreadable/malformed
manifest.

## The report

Self-contained HTML: zero JavaScript, zero external requests, inline SVG
sparklines. Sections: header (period, host, targets with resolver provenance,
clock note, retention), period summary (per-target probes / OK / failed /
availability / outages), per-target availability + latency + sparkline (red
bars = failed buckets), outage table (start, duration, layers, cause,
location, flap), footer (generator + optional SHA-256 manifest name).

- **Availability** per target over the period.
- **Outage windows** open after `fail_threshold` consecutive failures,
  backdated to the first failure; closed on the first success.
- **Cause** = the most frequent failure detail, e.g.
  `dns:query-timeout@1.1.1.1`.
- **Location** = WiFi-vs-upstream from the gateway probe: `local` (gateway
  also failing), `upstream` (gateway fine), `unknown` (no gateway data).
- **Flap** = window shorter than 2× interval, or ≥3 closes within an hour.
- **Manifest** (`--manifest`) = SHA-256 of the HTML (+ CSV);
  `urania verify report.html` re-checks it — attach the `.sha256` file with
  the report so the recipient can prove it was not altered in transit.

## Configuration

`~/.config/urania/config.toml` (create it to override; missing file =
built-in defaults). Data: `~/.local/share/urania/` (Linux) or
`~/Library/Application Support/urania/` (macOS). Each key in the file
overrides the default independently — but a file that contains any
`[[target]]` entries uses exactly those targets (the built-in seven are
not merged in). `urania config` shows the effective configuration.

```toml
fail_threshold = 3     # consecutive failures that open an outage window
retention_days = 90    # probe/outage retention
# db_path = "..."      # default: data dir / urania.db
# log_path = "..."     # default: data dir / urania-YYYY-MM-DD
# icmp = true          # enable ICMP targets (needs the `icmp` feature + permission)

[[target]]
name = "dns-cloudflare"
type = "dns"            # dns | tcp | http | icmp
endpoint = "1.1.1.1"    # dns: resolver IP; tcp: host:port or auto:443,80; http: URL
interval = 5            # seconds
timeout = 3             # seconds
```

Defaults (7 targets): DNS 1.1.1.1 + 8.8.8.8 (hand-rolled A query for
example.com over TCP/53), TCP 1.1.1.1:443, HTTP gstatic + cloudflare, gateway
`auto:443,80` (auto-detected default gateway, 443→80 fallback), ICMP 1.1.1.1
(skipped unless `icmp = true` and the build has the feature). The gateway is a
reference probe: it attributes outages to local link vs upstream, and if the
router has never answered TCP, its silence is recorded without an outage
claim (many home routers run no TCP 80/443 service).

## FAQ

- **ICMP permissions.** Linux needs root or CAP_NET_RAW; macOS usually works
  without prompts. Without permission the daemon logs a warning and skips ICMP
  targets — everything else runs.
- **Which resolver is measured?** The dns probe queries the resolver IP from
  the target config (default `1.1.1.1`), not the system resolver; each probe
  row records its resolver so the evidence states what was measured.
- **Clock.** Each probe stores wall time + the monotonic delta since the
  previous probe; the report flags probes whose delta deviates >10% from the
  expected interval (possible clock adjustment). The NTP state is recorded
  best-effort in the header.
- **`start` succeeded but `status` says not running.** Check the log in the
  data dir (daily rotation, keep 2).
- **The gateway section says "reference probe (no outage claims)".** Normal
  for routers that do not answer TCP 80/443. The gateway target exists to
  attribute outages to the local link vs upstream; silence from a gateway that
  has never responded is its baseline state, not an outage. Once the gateway
  has answered at least once, sustained silence is treated as a real
  local-link outage.
- **systemd DynamicUser.** The db is owned by the dynamic user; if `urania
  status` can't read it, use `systemctl status urania`.

## Non-goals (v1)

No GUI, no speed tests, no webhooks, no Windows, no auto-update.

## Rebrand note

This project was planned as `netproof`; see [REBRAND.md](REBRAND.md).
