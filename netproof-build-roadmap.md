# netproof — Max-Detail Build Roadmap

- **Date:** 2026-10-15
- **Input:** [rust-small-project-final-verdict.md](rust-small-project-final-verdict.md) (GO, unconditional) + [rust-small-project-deep-analysis.md](rust-small-project-deep-analysis.md) (spec B/C pillars) + [rust-small-project-deep-research.md](rust-small-project-deep-research.md) (origin)
- **Status:** Decision-complete build plan — every day, file, type, schema, test, and artifact specified
- **Budget:** ~5 working days, ~2.4k LOC (code) + ~600 LOC tests, 4 static binaries

---

## 0. Goal & Success Criteria

**Goal:** Build and publish `netproof` v1 — a headless, zero-permission, single-static-binary Rust network-outage evidence logger (verdict §5 incl. deltas D1–D5).

**Positioning (locked, D4):** "no server, no port, no tray, no dashboard, zero-permission probes — one static binary. Rust is a footnote."
Tagline: *"netband tells you the network is bad right now. netproof proves it was bad last Tuesday."*

**Done = ALL of:**
1. `netproof selftest` passes on Linux (musl static) and macOS.
2. 24h unattended run: no crash, retention enforced, report renders.
3. HTML report fully self-contained (zero external requests; verified in browser devtools).
4. CI green: `cargo fmt --check`, `clippy -D warnings`, all tests — ubuntu + macOS.
5. Published: crates.io `netproof` + GitHub Release (4 static binaries + SHA-256 checksums) — **after passing the T-0 gate (§8)**.

---

## 1. Locked Decisions

### 1.1 Inherited from the verdict (no re-debate)

Probe layer DNS/TCP/HTTP (ICMP optional, off by default) · detection = outage windows + per-layer attribution + flap + WiFi-vs-upstream · SQLite 90-day retention + clock integrity · self-contained HTML + CSV report, multi-target defaults, optional SHA-256 manifest · CLI `start|stop|status|report|selftest` · no local server · no root by default · Linux + macOS only · non-goals: no GUI, no speed tests, no webhooks, no Windows, no auto-update.

### 1.2 New decisions (made here, all reversible)

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| N1 | License | MIT (single) | Simplest for a small tool |
| N2 | DNS probe | Hand-rolled minimal A-query over **TCP 53** (~150 LOC incl. tests) | Zero deps, full packet control (evidence integrity), TCP avoids UDP-53 ISP blocking (D.2.3); resolver IP recorded per probe (D2) |
| N3 | HTTP client | `reqwest` + `rustls-tls`, no default features | Static, no CGO, no OpenSSL |
| N4 | SQLite | `rusqlite` + `bundled` | Bundles C source → fully static on musl |
| N5 | Daemon model | Hidden `netproof run --foreground` is the real loop; `start` spawns it detached (setsid) + pidfile lock; systemd/Docker call `run --foreground` directly | One code path for all deployment modes (netband's proven pattern) |
| N6 | Gateway auto-detect | Linux: parse `ip -4 route show default`; macOS: parse `route -n get default`; best-effort, config-overridable; gateway probe = TCP 443, 2s timeout; no answer → `location_class = unknown` | No root, no ICMP; imperfection degrades to "unknown", never a wrong claim |
| N7 | Report JS | Zero JavaScript; period chosen at render time via `--since` | Static evidence artifact must not depend on runtime environment |
| N8 | NTP status | Best-effort at daemon start: Linux parses `timedatectl`; macOS = "unknown"; stored in `meta.clock_note` | B.3.3 without platform sprawl |

---

## 2. Architecture

### 2.1 Repo layout

```
netproof/
├── Cargo.toml
├── README.md                  # positioning copy per D4 (draft in §11.7)
├── LICENSE                    # MIT
├── CHANGELOG.md
├── PUBLISH-GATE.md            # T-0 gate evidence, written day 5
├── Dockerfile                 # D3 — scratch image, static binary
├── systemd/netproof.service   # D3
├── .github/workflows/
│   ├── ci.yml
│   └── release.yml
└── src/
    ├── main.rs                # entry; clap dispatch
    ├── cli.rs                 # clap definitions
    ├── config.rs              # Config, TOML load, defaults, CLI overrides
    ├── probe/
    │   ├── mod.rs             # Probe trait, ProbeResult, scheduler
    │   ├── tcp.rs
    │   ├── http.rs
    │   ├── dns.rs             # hand-rolled A query over TCP 53
    │   └── icmp.rs            # #[cfg(feature = "icmp")]
    ├── detect/
    │   ├── mod.rs             # Detector state machine
    │   ├── window.rs          # outage open/close
    │   ├── attr.rs            # per-layer attribution
    │   └── flap.rs            # flap detection
    ├── store/
    │   ├── mod.rs             # open/migrate/insert/query
    │   ├── schema.sql         # include_str!
    │   └── retention.rs       # prune + incremental_vacuum
    ├── report/
    │   ├── mod.rs
    │   ├── html.rs            # self-contained renderer + inline SVG sparklines
    │   ├── csv.rs
    │   └── manifest.rs        # SHA-256
    ├── daemon/
    │   ├── mod.rs             # start / stop / status / run_foreground
    │   ├── lock.rs            # pidfile + fs2
    │   ├── signals.rs         # SIGTERM/SIGINT graceful
    │   └── log.rs             # tracing-appender rolling
    └── selftest.rs            # probe→detect→store→report round-trip
```

### 2.2 Data flow

```
scheduler (per-target tokio intervals)
  → ProbeResult ──mpsc──→ Detector (per-target state machines)
                              ├─ InsertProbe ──→ Store (batched, WAL)
                              └─ OpenWindow / CloseWindow ──→ Store
report: SQL over [from, to] → HTML (sparklines, outage table, availability %) + CSV + optional manifest
```

Invariant: pidfile lock acquired before the probe loop starts; interactive `start`, systemd, and Docker all run the same `run --foreground` loop.

### 2.3 Core types (signatures)

```rust
// probe/mod.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ProbeType { Dns, Tcp, Http, Icmp }

pub struct ProbeResult {
    pub ts_wall_ms: i64,        // unix ms
    pub ts_mono_delta_ms: i64,  // monotonic delta since previous result for this target
    pub target: String,
    pub probe_type: ProbeType,
    pub ok: bool,
    pub latency_ms: Option<f64>,
    pub detail: String,         // "" on success; error class / resolver / stage / status on failure
}

#[async_trait]
pub trait Probe: Send + Sync {
    fn target(&self) -> &Target;
    async fn run(&self) -> ProbeResult;
}

// Scheduler: one tokio::time::interval per target; each round = join_all(probes);
// results sent over mpsc::Sender<ProbeResult>; cancellation via oneshot handle.

// detect/mod.rs
pub enum StoreCmd {
    InsertProbe(ProbeResult),
    OpenWindow { target: String, start_ts: i64 },
    CloseWindow { id: i64, end_ts: i64, layers: String, cause: String,
                  location_class: String, flap: bool, probes_failed: u32 },
}

pub struct Detector {
    cfg: DetectCfg,                       // fail_threshold, flap window params
    states: HashMap<String, TargetState>, // per target
    gateway: Option<GatewayState>,        // N6
}

pub struct TargetState {
    consecutive_failures: u32,
    first_fail_ts: Option<i64>,
    open_window: Option<WindowBuilder>,
    recent_closes: VecDeque<i64>,         // flap tracking (rolling 1h)
}

pub struct WindowBuilder {
    id: i64,
    failing_types: BTreeSet<ProbeType>,   // attribution input
    detail_samples: Vec<String>,          // finest cause (e.g. "dns:query-timeout@1.1.1.1")
    gateway_ok_in_window: u32,
    gateway_fail_in_window: u32,
    probes_failed: u32,
}

// store/mod.rs
pub struct Store { conn: Connection }
impl Store {
    pub fn open(path: &Path) -> Result<Self>;                    // WAL, busy_timeout=5000, migrations
    pub fn insert_probes(&self, rows: &[ProbeResult]) -> Result<()>;
    pub fn open_window(&self, target: &str, start_ts: i64) -> Result<i64>;
    pub fn close_window(&self, id: i64, end_ts: i64, layers: &str,
                        cause: &str, loc: &str, flap: bool, n: u32) -> Result<()>;
    pub fn set_meta(&self, k: &str, v: &str) -> Result<()>;
    pub fn get_meta(&self, k: &str) -> Result<Option<String>>;
    pub fn prune(&self, retention_days: u32) -> Result<u32>;     // deleted rows; incremental_vacuum
    pub fn last_probe(&self, target: &str) -> Result<Option<ProbeRow>>;
    pub fn open_windows(&self) -> Result<Vec<WindowRow>>;        // for restart re-open
    pub fn probes_in(&self, from_ms: i64, to_ms: i64) -> Result<Vec<ProbeRow>>;
    pub fn outages_in(&self, from_ms: i64, to_ms: i64) -> Result<Vec<OutageRow>>;
}

// report/mod.rs
pub struct ReportOpts {
    pub since: Period,                 // 24h | 7d | 30d | 90d (default 24h)
    pub out_html: PathBuf,
    pub out_csv: Option<PathBuf>,
    pub manifest: bool,
}
pub fn render(cfg: &Config, store: &Store, opts: &ReportOpts) -> Result<RenderSummary>;

// report/html.rs — pure functions over fixture data (testable pre-daemon):
fn sparkline_svg(samples: &[(f64, bool)], w: u32, h: u32) -> String  // ≤96 downsampled buckets
fn availability_pct(rows: &[ProbeRow]) -> f64
fn latency_stats(rows: &[ProbeRow]) -> (f64, f64, f64, f64)          // avg, min, max, jitter(stddev)

// report/manifest.rs
pub fn write_manifest(paths: &[PathBuf]) -> Result<PathBuf>;         // "<sha256hex>  <name>" lines

// daemon/lock.rs
pub struct PidLock { file: File, path: PathBuf }
impl PidLock {
    pub fn acquire(path: &Path) -> Result<Self>;  // stale detection: pid alive? is it netproof?
    pub fn release(self) -> Result<()>;
}

// daemon/mod.rs
pub fn start(cfg: &Config) -> Result<()>;          // spawn detached `run --foreground` (setsid pre_exec), verify after 500ms
pub fn run_foreground(cfg: &Config) -> Result<()>; // lock → store(migrate, re-open stale windows) → tasks → signal loop
pub fn stop(cfg: &Config) -> Result<()>;           // SIGTERM, wait ≤10s, report; no SIGKILL in v1
pub fn status(cfg: &Config) -> Result<()>;         // works with daemon down (last known data)
```

### 2.4 Dependencies (all static, no CGO)

| Crate | Features / notes |
|-------|------------------|
| clap 4 | `derive` |
| tokio 1 | `rt-multi-thread, macros, time, net, signal, process, io-util, fs` |
| rusqlite | `bundled` |
| reqwest | `default-features = false`, `rustls-tls`, `http2` |
| chrono 0.4 | `serde` |
| serde 1 + toml 0.8 | config |
| tracing + tracing-subscriber + tracing-appender | `env-filter`; rolling logs |
| thiserror 2 + anyhow 1 | errors |
| csv 1 | CSV export |
| sha2 0.10 | manifest |
| fs2 0.4 | pidfile lock |
| async-trait | `Probe` trait |
| ping (feature `icmp` only) | off by default; runtime capability check |

### 2.5 Platform matrix & static build

| Target | Runner | Notes |
|--------|--------|-------|
| x86_64-unknown-linux-musl | ubuntu-latest + `musl-tools` | primary server target |
| aarch64-unknown-linux-musl | ubuntu-24.04-arm + `musl-tools` | native arm runner (no cross linker needed) |
| aarch64-apple-darwin | macos-latest (arm) | M-series |
| x86_64-apple-darwin | macos-latest | Intel macs |

`rusqlite/bundled` + rustls ⇒ no system deps. Acceptance: `file netproof-<target>` reports "statically linked" on musl targets.

---

## 3. Data Model

`src/store/schema.sql` (embedded via `include_str!`, versioned in `meta`):

```sql
PRAGMA journal_mode = WAL;

CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
-- keys: schema_version, created_at, tool_version, clock_note, resolver_note

CREATE TABLE probes (
  id               INTEGER PRIMARY KEY,
  ts               INTEGER NOT NULL,   -- unix ms, wall clock
  ts_mono_delta_ms INTEGER,            -- monotonic delta since previous probe (clock integrity)
  target           TEXT NOT NULL,
  probe_type       TEXT NOT NULL,      -- dns | tcp | http | icmp
  ok               INTEGER NOT NULL,   -- 0/1
  latency_ms       REAL,
  detail           TEXT                -- dns: "resolver=1.1.1.1 stage=connect|query rcode=NOERROR|SERVFAIL"
                                       -- http: "status=204" / "tls=..." ; tcp: "timeout|refused|unreachable"
);
CREATE INDEX idx_probes_ts ON probes(ts);
CREATE INDEX idx_probes_target_ts ON probes(target, ts);

CREATE TABLE outages (
  id             INTEGER PRIMARY KEY,
  start_ts       INTEGER NOT NULL,
  end_ts         INTEGER,              -- NULL = ongoing
  target         TEXT NOT NULL,
  layers         TEXT NOT NULL,        -- semicolon-joined failing probe types, e.g. "dns;tcp"
  cause          TEXT,                 -- finest attribution, e.g. "dns:query-timeout@1.1.1.1"
  probes_failed  INTEGER NOT NULL,
  location_class TEXT                  -- local | upstream | unknown
);
```

**Paths (defaults):**

| Item | Linux (user) | Linux (systemd) | macOS |
|------|--------------|-----------------|-------|
| DB + lock + logs | `~/.local/share/netproof/` | `/var/lib/netproof/` (`StateDirectory=netproof`) | `~/Library/Application Support/netproof/` |
| Config | `~/.config/netproof/config.toml` | same | same |

DB file `netproof.db`; WAL mode; `busy_timeout = 5000`.

---

## 4. CLI & Config Spec

### 4.1 Commands

```
netproof start  [--config PATH] [--foreground]   # spawn detached daemon (or run in place)
netproof stop                                     # SIGTERM, wait ≤10s
netproof status                                   # fixed-shape output below
netproof report [--since 24h|7d|30d|90d] [--to FILE.html] [--csv FILE.csv] [--manifest]
netproof once   [--target T] [--type dns|tcp|http]   # D1: one round, print table, exit 0/1
netproof selftest                                   # round-trip vs known-good targets, temp DB
netproof run --foreground                           # hidden; real daemon loop (systemd/Docker)
```

Exit codes: `once` → 0 all ok / 1 any failed; `selftest` → 0 PASS / 1 FAIL; `start` → 0 started / 2 already running; `report` → 0 ok / 3 no data in period.

### 4.2 `status` output (fixed shape)

```
netproof v0.1.0
  running:   yes (pid 4242, uptime 3d 4h)
  db:        /var/lib/netproof/netproof.db (12.4 MB, retention 90d, 384,211 rows)
  lock:      held by pid 4242
  targets:
    dns-cloudflare   last 13:00:01 ok   12.1ms  (1.1.1.1, TCP/53, 5s)
    tcp-cloudflare   last 13:00:01 ok   11.8ms  (1.1.1.1:443, 5s)
    http-gstatic     last 13:00:10 ok   48.3ms  (204, 10s)
    gateway          last 13:00:01 ok    0.4ms  (192.168.1.1:443 auto, 5s)
  open outages: none
```

### 4.3 Config file (`~/.config/netproof/config.toml`)

```toml
fail_threshold = 3            # consecutive failures to open a window
retention_days = 90
db_path   = ""                # "" = platform default
log_path  = ""
icmp = false                  # feature-gated; capability-checked even when true

[[target]]
name = "dns-cloudflare"
type = "dns"
endpoint = "1.1.1.1"          # resolver probed over TCP 53 for "example.com"
interval = 5
timeout = 3

[[target]]
name = "dns-google"
type = "dns"
endpoint = "8.8.8.8"
interval = 5
timeout = 3

[[target]]
name = "tcp-cloudflare"
type = "tcp"
endpoint = "1.1.1.1:443"
interval = 5
timeout = 3

[[target]]
name = "tcp-google-53"
type = "tcp"
endpoint = "8.8.8.8:53"
interval = 5
timeout = 3

[[target]]
name = "http-gstatic"
type = "http"
endpoint = "https://connectivitycheck.gstatic.com/generate_204"
interval = 10
timeout = 5

[[target]]
name = "http-google"
type = "http"
endpoint = "https://www.google.com/generate_204"
interval = 10
timeout = 5

[[target]]
name = "gateway"
type = "tcp"
endpoint = "auto:443"         # N6 auto-detect; feeds location_class
interval = 5
timeout = 2
```

Defaults = the above (multi-target, anti single-target bias — B.4.3). Equilibrium per verdict §4: 5–10s intervals, ICMP off, 90-day retention.

### 4.4 Probe success criteria

| Type | Success | Failure `detail` classes |
|------|---------|--------------------------|
| DNS | A answer (RCODE NOERROR, ≥1 A record) for `example.com` via TCP 53 to configured resolver | `connect`, `query-timeout`, `rcode=SERVFAIL|NXDOMAIN|...`, `no-answers` |
| TCP | handshake within timeout | `timeout`, `refused`, `unreachable`, `dns-resolution-failed` |
| HTTP | 2xx within timeout (204 expected on default endpoints) | `timeout`, `status=<code>`, `tls=<reason>`, `connect=<reason>` |
| ICMP | echo reply within timeout (feature) | `timeout`, `permission` |

---

## 5. Day-by-Day Build Plan

### Day 0 — Scaffold (~1h)

1. `cargo new netproof --bin`; `LICENSE` (MIT); `.gitignore` (`/target`, `*.db*`); `CHANGELOG.md` stub.
2. `Cargo.toml` exactly per §11.1 (deps, features, release profile).
3. `src/` module skeleton — every file exists with `todo!()` bodies; `cargo check` green from minute one.
4. CI skeleton `.github/workflows/ci.yml` per §11.5 (fmt + clippy + test, ubuntu + macOS).
5. First commit; push; verify CI runs.

**Exit criterion:** CI green on an empty-but-compiling tree.

### Day 1 — Probe layer + `--once`

1. `config.rs`: `Config` struct (`#[derive(Deserialize)]`), `default_config()`, `load(path)`, `merge_cli_overrides`, validation (unknown probe type, unparseable endpoint, `interval < 1` → typed errors via `thiserror`).
2. `probe/mod.rs`: `ProbeResult`, `Probe` trait, `Scheduler` (per-target intervals, parallel rounds, mpsc output, oneshot cancellation).
3. `probe/tcp.rs`: `tokio::net::TcpStream::connect` + timeout; classify errors per §4.4.
4. `probe/http.rs`: reqwest GET + timeout; record status code; TLS errors recorded distinctly (evidence: "TLS failed" ≠ "timeout").
5. `probe/dns.rs`: `build_a_query(domain) -> Vec<u8>` (header + question, fixed ID, RD=1); `send_tcp(query, resolver) -> io::Result<Vec<u8>>` (2-byte length prefix); `parse_response(bytes) -> Result<DnsAnswer>` (RCODE + answer count); `detail` = `resolver=<ip> stage=<connect|query> rcode=<...>` — **this is D2 resolver provenance**.
6. `probe/icmp.rs` behind `#[cfg(feature = "icmp")]`: `ping` crate; capability check (Linux CAP_NET_RAW/root; macOS firewall prompt); unavailable → log once, skip target, never crash.
7. `--once` (D1): one round, aligned table, exit 0/1.
8. **Tests:**
   - TCP: local `tokio` listener — ok / timeout (blackhole via unrouteable 10.255.255.1) / refused (closed port).
   - HTTP: local test server — 204 ok / 500 fail / plain-HTTP-to-HTTPS endpoint → `tls=` detail.
   - DNS: `build_a_query` → exact expected bytes (golden fixture); `parse_response` against fixed byte strings: valid A answer, SERVFAIL, truncated, empty answers.
   - Config: valid TOML, missing file → defaults, bad type → typed error.
9. **Exit criterion:** `netproof once` on a live box prints a correct 7-row table; probe unit tests green.

### Day 2 — Detection + storage

1. `store/mod.rs`: `open` (WAL, busy_timeout, migration runner keyed on `meta.schema_version`; v1 = `schema.sql`); batched inserts (flush per round or every 100 rows, single transaction).
2. `detect/window.rs`: per-target state machine — consecutive-failure counter; `first_fail_ts` tracked; `fail_threshold` (default 3) opens window at the **first** failure of the run (back-dated, correct evidence); first success closes (`end_ts`).
3. `detect/attr.rs`: at close, from `WindowBuilder.failing_types` + `detail_samples`: all types → `link`; dns-only → `dns`; tcp ok + http fail → `http/edge`; finest cause = most frequent `detail` sample.
4. `detect/flap.rs`: window duration < 2× target interval → flap candidate; or ≥3 closes within rolling 1h (`recent_closes` deque) → `flap = true`.
5. Location class (N6): at close, from `gateway_ok_in_window` / `gateway_fail_in_window`: gateway failing + externals failing → `local`; gateway ok + externals failing → `upstream`; gateway "unknown" (auto-detect failed or no answers) → `unknown`.
6. `store/retention.rs`: on daemon start + daily tick: `DELETE FROM probes WHERE ts < ?`, close orphaned outages, `PRAGMA incremental_vacuum`; log deleted count.
7. Clock integrity (B.3.3): every `ProbeResult` carries `ts_mono_delta_ms`; at start write `meta.clock_note` (N8: `timedatectl` parse on Linux, "unknown" on macOS); at report time flag probes whose mono delta deviates >10% from expected interval as "possible clock adjustment".
8. Restart recovery: on `run --foreground` start, `open_windows()` → re-open windows whose last underlying probe state was failing (from `last_probe` per target).
9. **Tests (all offline, synthetic streams — no network):**
   - clean stream → 0 windows
   - 4 consecutive fails → 1 window, start = first fail ts, end = recovery ts
   - dns-only failure → attribution `dns`, layers `dns`
   - all-types failure + gateway failing → `local`; gateway ok → `upstream`; gateway unknown → `unknown`
   - 4 short windows in 1h → flap flag
   - restart mid-outage → window re-opened
   - retention prune on in-memory DB (insert 100d of rows, prune 90d, assert counts + vacuum)
10. **Exit criterion:** detection suite green with zero network access; probe→store round-trip verified in a test.

### Day 3 — Report + selftest ⚠️ (verdict M7: the slip risk)

1. `report/csv.rs`: probe rows in period → CSV, deterministic columns `ts_iso,target,type,ok,latency_ms,detail`.
2. `report/html.rs` — the artifact:
   - **Header block:** tool name+version, period, generation timestamp (with offset), host (hostname + OS), target list with intervals, **resolver provenance per DNS target (D2)**, clock note + NTP status + drift flags, retention info.
   - **Per target:** availability % (ok/total), latency stats (avg/min/max/jitter=stddev), inline-SVG sparkline — hand-rolled: downsample to ≤96 buckets, `polyline` for latency, red `<rect>` background segments for failure intervals; no chart lib, no JS.
   - **Outage table:** start, duration, layers, cause, location class, flap flag.
   - **Footer:** "self-contained artifact · generated by netproof vX · sha256: …" (when manifest requested).
   - Single file, inline CSS only, zero external requests, zero JS (N7).
3. `report/manifest.rs`: `--manifest` writes `<file>.sha256` (hex + filename) over CSV and HTML bytes.
4. `selftest.rs`: one round vs known-good public targets (1.1.1.1:53 TCP, gstatic 204, DNS 1.1.1.1) → temp DB → detector (expect 0 windows) → render HTML to temp path → assert: all probes ok, 0 outages, HTML contains header + sparkline + every target row; print PASS/FAIL. Reuses `--once` internals (D1).
5. **Tests (pure functions over fixtures):** 100 probe rows + 2 outages → assert outage rows, availability % values, sparkline `points` count, resolver line present, zero external URLs in output (regex `src=|href=http` must not match).
6. **Contingency (pre-committed):** if sparkline rendering overruns by >3h, ship v0.1.0 with table-only report and land sparklines in v0.1.1. The table is the minimum viable evidence artifact; the sparkline is polish.
7. **Exit criterion:** `netproof selftest` PASS; `netproof report --since 24h` opens in a browser with zero network requests (devtools-verified).

### Day 4 — Daemon lifecycle + deployment artifacts (D3)

1. `daemon/lock.rs`: pidfile in data dir + `fs2` `lock_exclusive`; stale detection (pid alive via `kill -0`? is it netproof via `/proc/<pid>/cmdline` on Linux / `sysctl` on macOS?); `start` refuses with lock owner message.
2. `daemon/mod.rs`:
   - `start`: spawn `netproof run --foreground` detached (`pre_exec` setsid; stdout/err → rolling log file), write pidfile, verify liveness after 500ms.
   - `run --foreground`: lock → open store (migrate, re-open stale windows, retention tick) → start scheduler + detector + retention tasks → signal loop.
   - `stop`: pidfile → SIGTERM → wait ≤10s → report; **no SIGKILL in v1** (graceful only, documented).
   - `status`: per §4.2; works with daemon down (reports "not running" + last known data).
3. `daemon/signals.rs`: SIGTERM/SIGINT → cancel scheduler, close open windows (`end_ts = now`), flush + close DB, one final log line.
4. `daemon/log.rs`: `tracing-appender` rolling, 5MB, keep 2; env-filter.
5. `systemd/netproof.service` per §11.4.
6. `Dockerfile` per §11.6; README note: container resolver is the measurement target (M5/D2), `/etc/resolv.conf` runtime-mounted.
7. **Integration test (CI):** local dummy targets (local TCP listener + local HTTP 204 server on 127.0.0.1) → `run --foreground` → wait 3 rounds → SIGTERM → `report` → assert HTML + CSV exist and parse.
8. **Manual checks (one box):** systemd start/stop/restart; kill -9 recovery (stale lock + re-opened window); Docker run + report.
9. **Exit criterion:** start→stop→status→report works interactively, under systemd, in Docker, and in CI; kill -9 recovery verified.

### Day 5 — Publish

1. **T-0 gate (load-bearing, verdict §7):** run the Appendix A scripted gap scan verbatim (§11.8) + fetch netband README/changelog for attribution/report features → write results into `PUBLISH-GATE.md`. **If the evidence wedge is occupied → stop, pivot to envdrift. No exceptions.**
2. `README.md` per §11.7 (D4 copy) + install (cargo / release binary / systemd / Docker) + report screenshot + FAQ (ICMP permissions, Docker resolver, clock).
3. Crate metadata: description, 5 keywords, categories, repository, license.
4. `cargo publish`; tag `v0.1.0`; release workflow builds 4 targets + SHA-256 checksums + attaches.
5. Verify on a clean machine/container: `cargo install netproof` → `selftest` → `start` → `report`.
6. Announce (same day): r/sysadmin, r/rust, Hacker News "Show HN" — copy leads with the attachable-report hook, not the language.

**LOC budget (code, excl. tests):** cli/config 250 · probe 650 · detect 350 · store 300 · report 500 · daemon 300 · selftest 120 ≈ **2,470** (+ ~600 test LOC).

---

## 6. Testing Strategy

| Layer | What | Network? |
|-------|------|----------|
| Unit | DNS packet golden fixtures; detection state machine (clean/outage/flap/dns-only/gateway-down/restart); retention prune; renderer fragments + no-external-URL regex; config validation | No |
| Unit | TCP/HTTP probes vs local listeners/test servers | Local only |
| Integration (CI) | daemon start→probe(local dummies)→stop→report round-trip | Local only |
| Live | `selftest` vs public targets (manual + tag workflow) | Yes, gated |

CI: `fmt --check` + `clippy -D warnings` + `test --all-features` on ubuntu-latest & macos-latest, every push. Tag workflow: 4-target release build.

---

## 7. Edge Cases & Failure Modes

| Case | Behavior |
|------|----------|
| Network down at daemon start | Probes log failures; no crash; window opens at threshold |
| Wall-clock jump (NTP step) | `ts_mono_delta_ms` recorded; report clock note flags >10% deviations |
| DB locked / corrupt | WAL + busy_timeout; on corruption: clear error, exit non-zero, log path printed (no silent data loss) |
| DNS resolver itself down | `detail` distinguishes connect-fail vs query-timeout vs RCODE → attribution "link to resolver", not "DNS broken" |
| Container resolver ≠ host apps | Resolver provenance in report header (D2); Dockerfile documents the vantage point |
| Second `netproof start` | Refused with pid + lock owner message (exit 2) |
| Disk full | Insert error logged, probing continues, `status` shows warning |
| Gateway probe gets no 443 answer | `location_class = unknown` — never a false claim |
| kill -9 the daemon | Stale lock detected on next start; open windows re-opened from last probe state |
| Target renamed in config | Old rows stay (target is a string key); report shows both under their names |

---

## 8. Gates & Post-Publish (verdict §7, unchanged)

| Gate | When | Test | Action on failure |
|------|------|------|-------------------|
| **T-0** | day 5, pre-publish | Appendix A scan (§11.8) + netband check | wedge occupied → **stop, pivot to envdrift** |
| **T+30** | 30d post-publish | downloads < 200 AND stars < 10 AND zero organic mentions → weak; feedback uniformly "where's the GUI?" → wrong wedge | weak → pause/reposition; wrong → **pivot to envdrift** |
| **T+90** | 90d | netband (or any entrant) ships attribution + attachable report; or internet-monitor revives headless | stop competing head-on; reassess residual differentiation (zero-permission probes, macOS, Rust distribution) |

---

## 9. Everything Required (checklist)

1. Rust stable toolchain + `rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl`
2. CI: ubuntu-latest (`musl-tools` apt) + ubuntu-24.04-arm (`musl-tools` apt) + macos-latest
3. GitHub repo + Actions
4. crates.io account + `CARGO_REGISTRY_TOKEN` (publish + release secret)
5. One live Linux box + one macOS box for the 24h soak + systemd/Docker manual tests
6. No runtime system dependencies — that is the product
7. ~5 working days

---

## 10. Acceptance Criteria (v1 done)

1. `netproof selftest` PASS on Linux + macOS.
2. 24h unattended: no crash, retention enforced, report renders.
3. HTML report: zero external requests (devtools), opens offline.
4. CI green both OSes; `clippy -D warnings`; `file` shows "statically linked" on musl targets.
5. Published: crates.io + release (4 binaries + checksums) after documented T-0 pass.
6. README copy follows D4 (no-server lead, Rust footnote).

---

## 11. Appendices — Verbatim Artifacts

### 11.1 `Cargo.toml`

```toml
[package]
name = "netproof"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Headless, zero-permission network outage evidence logger: probes DNS/TCP/HTTP, logs to SQLite, renders an attachable self-contained HTML report."
keywords = ["network", "monitoring", "outage", "diagnostics", "isp"]
categories = ["network-programming", "command-line-utilities"]
repository = "https://github.com/<you>/netproof"

[features]
default = []
icmp = ["dep:ping"]

[dependencies]
anyhow = "1"
async-trait = "0.1"
clap = { version = "4", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
csv = "1"
fs2 = "0.4"
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "http2"] }
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
sha2 = "0.10"
thiserror = "2"
toml = "0.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "net", "signal", "process", "io-util", "fs"] }
tracing = "0.1"
tracing-appender = "0.2"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[target.'cfg(feature = "icmp")'.dependencies]
ping = "0.4"

[profile.release]
lto = true
codegen-units = 1
strip = true
```

### 11.2 `src/store/schema.sql`

See §3 (single source of truth; embed via `include_str!("store/schema.sql")`).

### 11.3 `systemd/netproof.service`

```ini
[Unit]
Description=netproof network evidence logger
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/bin/netproof run --foreground
DynamicUser=true
StateDirectory=netproof
Restart=on-failure
RestartSec=5
# No open ports, no capabilities needed: probes are plain TCP/HTTP/DNS.

[Install]
WantedBy=multi-user.target
```

Install: `sudo cp netproof /usr/bin/ && sudo cp systemd/netproof.service /etc/systemd/system/ && sudo systemctl enable --now netproof`.

### 11.4 `Dockerfile`

```dockerfile
# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM rust:1-stable-musl AS build
WORKDIR /src
COPY . .
ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
      x86_64) export TARGET=x86_64-unknown-linux-musl ;; \
      arm64)  export TARGET=aarch64-unknown-linux-musl ;; \
    esac && rustup target add "$TARGET" && \
    cargo build --release --target "$TARGET"

FROM scratch
COPY --from=build /etc/ssl/certs /etc/ssl/certs
COPY --from=build /usr/share/zoneinfo /usr/share/zoneinfo
COPY --from=build /src/target/*/release/netproof /netproof
# /etc/resolv.conf is mounted by the runtime; the container's resolver is the
# measurement target and is recorded in the report header (resolver provenance).
ENTRYPOINT ["/netproof"]
CMD ["run", "--foreground"]
```

Run: `docker run -d --name netproof -v netproof-data:/root/.local/share/netproof <image>`; report out: `docker exec netproof netproof report --since 24h --to /tmp/r.html && docker cp netproof:/tmp/r.html .`

### 11.5 `.github/workflows/ci.yml`

```yaml
name: ci
on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo fmt --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test --all-features
```

`.github/workflows/release.yml`:

```yaml
name: release
on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-musl
          - os: ubuntu-24.04-arm
            target: aarch64-unknown-linux-musl
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: macos-latest
            target: x86_64-apple-darwin
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Install musl tools (Linux)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y musl-tools
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - name: Package + checksum
        run: |
          cp target/${{ matrix.target }}/release/netproof "netproof-${{ matrix.target }}"
          sha256sum "netproof-${{ matrix.target }}" > "netproof-${{ matrix.target }}.sha256"
      - uses: softprops/action-gh-release@v2
        with:
          files: |
            netproof-*
            netproof-*.sha256
```

### 11.6 Report HTML skeleton (structure, not final CSS)

```html
<!doctype html>
<html><head><meta charset="utf-8"><title>netproof report — 24h</title>
<style>/* inline only: system font stack; table borders; .fail{background:#fdecea} */</style></head>
<body>
<header>
  <h1>netproof v0.1.0 — network evidence report</h1>
  <dl>
    <dt>Period</dt><dd>2026-10-14 13:00 → 2026-10-15 13:00 (24h)</dd>
    <dt>Generated</dt><dd>2026-10-15 13:00:02 +08:00</dd>
    <dt>Host</dt><dd>box1, Linux 6.9 x86_64</dd>
    <dt>Targets</dt><dd>dns→1.1.1.1 (resolver 1.1.1.1, TCP/53, 5s); tcp→1.1.1.1:443 (5s);
      http→connectivitycheck.gstatic.com (10s); gateway→192.168.1.1:443 (auto, 5s); …</dd>
    <dt>Clock</dt><dd>wall clock + monotonic delta recorded; NTP: active (timedatectl); no drift flags</dd>
  </dl>
</header>
<section>
  <h2>dns-cloudflare — availability 99.82%</h2>
  <svg viewBox="0 0 600 60" width="600" height="60">
    <rect class="fail" x="210" y="0" width="18" height="60"/>
    <polyline fill="none" stroke="#1a7f37" points="0,40 6,39 … 600,41"/>
  </svg>
  <p>latency avg 12.4ms · min 9.1 · max 88.2 · jitter 6.3ms</p>
</section>
<!-- repeat per target -->
<section>
  <h2>Outages</h2>
  <table>
    <tr><th>Start</th><th>Duration</th><th>Layers</th><th>Cause</th><th>Location</th><th>Flap</th></tr>
    <tr><td>2026-10-14 22:14:05</td><td>4m 12s</td><td>dns;tcp;http</td>
        <td>dns:query-timeout@1.1.1.1</td><td>upstream</td><td>no</td></tr>
  </table>
</section>
<footer>self-contained artifact · generated by netproof v0.1.0 · sha256: 3f2a…</footer>
</body></html>
```

### 11.7 README positioning copy (D4)

```markdown
# netproof

No server. No port. No tray. No dashboard. Zero-permission probes.

netproof is a headless network evidence logger for servers, Docker, CI, and
SSH sessions. It continuously probes DNS, TCP, and HTTP (ICMP optional),
records every result to a local SQLite database, detects outage windows with
per-layer cause attribution, and renders a self-contained HTML report — the
artifact you attach to an ISP or IT ticket.

> netband tells you the network is bad right now.
> netproof proves it was bad last Tuesday.

- **Zero permissions.** Default probes use plain TCP/HTTP/DNS — no root, no
  raw sockets, no firewall prompts.
- **No local server.** No open port, no browser, nothing to lock down.
- **One static binary.** Written in Rust; no runtime dependencies.
- **Evidence integrity.** Multi-target defaults, per-probe resolver
  provenance, wall clock + monotonic delta, optional SHA-256 manifest.
```

### 11.8 T-0 gap scan (verbatim from verdict Appendix A; re-run day 5)

```bash
UA="Mozilla/5.0 (gap-scan)"
# 1. Name availability — expect 404 + "crate `netproof` does not exist"
curl -s -H "User-Agent: $UA" -w "\nHTTP %{http_code}\n" https://crates.io/api/v1/crates/netproof
# 2. Rust-space scan — expect NO result that is (a) single-binary, (b) persistent outage
#    logging, (c) per-layer attribution or attachable report.
for q in "ping+logger" "outage" "network+monitor" "probe+logger" "latency+monitor" "internet+monitor" "ping"; do
  curl -s -H "User-Agent: $UA" "https://crates.io/api/v1/crates?q=$q&per_page=15" \
   | python3 -c "import json,sys; [print(c['name'],'|',str(c.get('updated_at'))[:10],'|',(c.get('description') or '')[:90]) for c in json.load(sys.stdin).get('crates',[])]"
done
# 3. Incumbent state — expect: no headless/CLI mode
curl -s -H "User-Agent: $UA" https://api.github.com/repos/FutureSolutionDev/internet-monitor/releases?per_page=3
curl -s -H "User-Agent: $UA" "https://api.github.com/repos/FutureSolutionDev/internet-monitor/commits?per_page=5"
curl -s -H "User-Agent: $UA" https://raw.githubusercontent.com/FutureSolutionDev/internet-monitor/master/README.md | grep -inE "cli|headless|docker|systemd"
# 4. Closest Rust entrant — expect: no per-layer attribution / outage windows / HTML report
curl -s -H "User-Agent: $UA" https://raw.githubusercontent.com/gregl83/netband/main/README.md
```

Record all outputs + a dated conclusion in `PUBLISH-GATE.md`.

---

## 12. Master Map

```
BUILD ROADMAP — netproof v1
├── 0 GOAL & SUCCESS CRITERIA ............ 5 done-definitions
├── 1 DECISIONS .......................... verdict-locked + N1–N8 (license, DNS hand-rolled, rustls,
│                                           bundled SQLite, daemon model, gateway, no-JS, NTP)
├── 2 ARCHITECTURE ....................... module map, data flow, core type signatures,
│                                           deps (13, all static), 4-target matrix
├── 3 DATA MODEL ......................... meta/probes/outages SQL, paths, WAL
├── 4 CLI & CONFIG ....................... 7 commands, exit codes, status shape, TOML, success criteria
├── 5 DAY-BY-DAY .......................... D0 scaffold → D1 probes+--once → D2 detect+store →
│                                           D3 report+selftest (slip risk + contingency) →
│                                           D4 daemon+systemd+Docker → D5 publish
├── 6 TESTING ............................ unit (offline) / local / CI integration / gated live
├── 7 EDGE CASES ......................... 10 failure modes with specified behavior
├── 8 GATES .............................. T-0 (load-bearing) / T+30 / T+90
├── 9 REQUIRED ........................... toolchain, runners, accounts, boxes, 5 days
├── 10 ACCEPTANCE ......................... 6 checks
└── 11 APPENDICES ........................ Cargo.toml, schema, systemd, Dockerfile, CI YAML,
                                            HTML skeleton, README copy, T-0 scan script
```
