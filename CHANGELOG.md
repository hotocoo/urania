# Changelog

All notable changes to urania are documented here.

## 0.1.0 (unreleased)

- Initial release: headless, zero-permission network outage evidence logger.
- Probes: DNS (hand-rolled A query over TCP/53, resolver recorded per probe),
  TCP, HTTP (rustls, no OpenSSL); ICMP optional behind the `icmp` feature.
- Gateway auto-detect (Linux `ip route`, macOS `route -n get default`) with
  443→80 fallback; imperfection degrades to `location_class = unknown`.
- Gateway is a reference probe: a router that has never answered TCP is
  recorded without an outage claim (many home routers run no TCP 80/443
  service), so the report never shows a false 0% availability or a permanent
  gateway outage. Once the gateway has answered at least once, sustained
  silence opens a window as usual. Restart recovery closes pre-existing
  reference-only gateway windows (cause `reference-only: …`). One live
  evidence DB was corrected on 2026-10-15 (false open gateway window closed
  with the reference-only cause).
- CLI: `once` honors the same rule — a gateway with no historical success
  prints `ref` (table) / `ref_only: true` (`--json`) and does not fail the
  round; no db history = fail-open, every failure counts.
- Clock honesty: `ts_mono_delta_ms` now measures between consecutive probe
  *starts* (was: probe ends), so a slow-but-successful response no longer
  swings the delta and no longer trips the report's >10% drift flag — the
  flag now reflects scheduler/clock health only. Found via the 24h soak:
  dns-cloudflare showed a 6.4% false-positive drift rate from 1.1.1.1
  response duration (0.5–3.6s), not clock instability.
- Detection: outage windows (backdated to first failure), per-layer cause
  attribution, flap detection, WiFi-vs-upstream location class.
- Storage: SQLite (WAL, bundled C source → fully static), 90-day retention,
  clock integrity (wall clock + monotonic delta per probe), restart recovery.
- Report: self-contained HTML (inline SVG sparklines, zero JavaScript, zero
  external requests), deterministic CSV, optional SHA-256 manifest, and
  `urania verify report.html` — re-hashes the report against the manifest so
  a recipient can prove the attached evidence was not altered.
- Performance: report rendering does a single probe-table scan (previously
  two) and partitions rows into targets in one pass (previously a
  filter+clone per target); output is byte-identical for a given DB
  snapshot (verified by old-vs-new diff).
- Report: period-summary table at the top (per-target probes / OK / failed /
  availability / outage count) — the 10-second read for a ticket reviewer.
  Reference-only gateways show `ref-only` instead of an availability
  percentage; targets with no data in the period show `—`.
- Report: new `--since 1h` period, for "what happened in the last hour"
  reports that pair with the `status` 1h availability column.
- Docker: the scratch image now sets `HOME=/root`, so the documented
  volume mount at `/root/.local/share/urania` is actually used (with
  HOME unset the data dir fell back to a CWD-relative path and the
  volume was silently unused). The Dockerfile no longer relies on
  BuildKit-only `$BUILDPLATFORM` — it builds with both the legacy
  builder and BuildKit. Verified: image builds, `selftest` passes in
  the container, the binary is statically linked, a live probe round
  succeeds, and the database + log persist to the mounted volume.
- CLI: new `urania config` — prints the effective configuration
  (source file or built-in defaults, resolved db/log/lock paths,
  thresholds, and every target with type/endpoint/interval/timeout).
  README documents the override semantics: a config file with any
  `[[target]]` entries replaces the built-in targets, it does not merge.
- Release: `SHA256SUMS` now covers the released tarballs (per-binary
  sums were indistinguishable — all binaries are named `urania`);
  tarballs include `LICENSE`; the x86_64-musl artifact is smoke-tested
  before packaging.
- CLI: `urania status --check` — prints only the state and exits 0 =
  running, 1 = not running, for cron/monitoring scripts (the full
  `status` output is human-facing and always exited 0).
- Test: the integration test now covers the full evidence loop —
  `report --manifest`, `verify` (exit 0 fresh, exit 1 tampered),
  `status --check` (0 running, 1 stopped), and `once --target`.
- Provenance: the report header/footer, the db `tool_version` meta,
  and `status`/`config` now record the exact build (version + git
  commit, via build.rs; version alone when built without git).
- Provenance (report): the header now also shows which build collected
  the data (Collected by, from the db tool_version meta) — matters
  after an upgrade: data written by one build, rendered by another,
  is labeled as such.
- Report: the summary table gains an Observed column (first → last
  probe per target in the period) — states when the logger was
  actually observing each target, next to the coverage percentage.
- CLI: `urania once --json` — machine-readable one-shot output
  (ts, all_ok, per-target ok/latency/detail) for scripts; the
  table output is unchanged.
- CLI: `status` now shows the daemon build (from the db) on the
  running line — after an upgrade the CLI version and the
  collecting daemon version can differ, and the difference is
  now visible.
- Docs: README "How the evidence holds up" — what a recipient can
  independently check (integrity, provenance, completeness, clock
  honesty, honest silence, method).
- Report: per-target latency line gains p50 / p95 / p99
  (nearest-rank over successful probes) — "normal" vs
  "incident" latency in one line.
- CLI: `report --from <RFC3339> [--end <RFC3339>]` — render an
  absolute incident window instead of a trailing period (the
  report header labels it "absolute window").
- CLI: `verify --quiet` — silent on success (exit code only),
  failures still print which file mismatched.
- Docs: RUST_LOG controls the daemon log level (default info).
- Test: the integration test now covers the absolute-window
  report path (--from, "absolute window" label, verify) and
  verify --quiet.
- CLI: the report stdout line now names the period
  (e.g. "… (3600 probes, 0 outages, absolute window)").
- CLI: `status --json` — machine-readable status for monitoring
  (version, running, pid, uptime, daemon build, db, lock, per-target
  last probe + 1h availability, open outages). Exit 0 = running,
  1 = not running. `once --json` now uses serde_json (already in
  the tree via reqwest; no new transitive deps).
- Daemon: probe batch now flushes to the db at most every 5 s
  (previously only on a detector command or at 100 rows — with
  long probe intervals that was hours of unflushed evidence on
  a crash). Bounded data loss: ≤5 s of probes.
- CLI: `config --json` — the effective configuration
  (source, db/log/lock paths, thresholds, all targets) as
  machine-readable JSON for config management and auditing.
- Release: the x86_64-musl release binary now runs the full
  selftest (local TCP dummy, probe, report, manifest, verify)
  before packaging. Every release also publishes a multi-arch
  GHCR image (linux/amd64 + linux/arm64), smoke-tested with
  selftest after push.
- CLI: `report --since` and `report --from` are now mutually
  exclusive with an explicit error (previously --from silently
  won, which could change the evidence window without the
  user noticing). --since is now optional (default 24h).
- CLI: `once --quiet` — silent on success (exit code
  only); failures still print (the exit code says what,
  the output says which).
- CLI: `selftest --offline` — the full evidence chain
  (probe, store, detector, report, manifest, verify)
  against a local TCP dummy, no network. For air-gapped
  installs.
- Daemon: the shutdown batch flush now logs how many
  probe rows it wrote, and reports a failure instead of
  silently swallowing it.
- CLI: `status --json` open outages now include
  `duration_s` (the text form always showed the elapsed
  time; the JSON form was missing it).
- Docs: the prebuilt-binary install path now shows the
  verify-first workflow (sha256sum -c, extract, install,
  selftest / selftest --offline) — an evidence tool you
  cannot authenticize is an evidence tool you cannot trust.
- Daemon: once a day the daemon writes a `heartbeat:` line
  (uptime + db row count) as a liveness record. A healthy
  daemon with no outages was previously silent between start
  and stop, so a hung daemon was indistinguishable from a
  quiet one.
- CLI: `report --stdout` — the HTML report to stdout for CI
  log attachments; the status line goes to stderr so
  `report --stdout > r.html` is clean. The evidence chain
  (--csv/--manifest) needs files on disk, so clap rejects
  the combination.
- CLI: `report --title "Incident …"` — a custom report
  title (incident label) in the header; the period is
  always shown separately, so the evidence window is never
  hidden by the label.
- Report: the outage count for a period now uses overlap
  semantics — a window counts if it started before the
  period's end and is still open or ended inside it.
  Previously only windows that *started* inside the period
  counted, so an outage that began before the period and is
  still running (the one that matters most) was missing
  from the report.
- Daemon: the single-instance lock now acquires the flock
  before truncating the pidfile, and uses a non-blocking
  try-lock. Previously a concurrently starting daemon could
  truncate the live daemon's pid out of the pidfile and
  then block on the lock — corrupting `status` while the
  daemon ran. Double-start now fails fast with the live
  pid in every case.
- Daemon: restart recovery now preserves the layer
  attribution recorded when an outage window opened
  (e.g. `tcp+dns`). Previously the recovery close paths
  overwrote it with `unknown`, losing evidence of which
  layers had failed.
- Release: `SHA256SUMS` now lists bare tarball filenames
  instead of `artifacts/<target>/` paths. The README's
  verify-first step (`sha256sum -c SHA256SUMS
  --ignore-missing`) would otherwise see every line as
  "missing" and pass silently — verifying nothing. Now the
  downloaded tarball is actually checked, and a tampered
  file still fails.
- CLI: `status --since 1h|24h|7d|30d|90d` — the per-target
  availability now covers the requested window instead of
  always 1h, for a quick health check over a longer period
  without generating a full report. The default is 1h, so
  existing output and the JSON key `avail_1h_pct` are
  unchanged; `--since 7d` exposes `avail_7d_pct`.
- CLI: `report --outages-csv <path>` — writes the
  outage-window summary (start/end/duration/target/layers/
  cause/location/flap) as a separate CSV, the attachable
  incident summary for an ISP dispute instead of the full
  per-probe CSV. An empty body (header only) is meaningful:
  it proves there were no outages in the period. Covered by
  the `--manifest` SHA-256 chain.
- Config: fail-fast validation for an empty target list, whitespace-only
  target names, and ICMP endpoints containing whitespace (previously
  accepted, then logged silence or failed obscurely at probe time).
- Daemon: single-instance flock + pidfile (no stale-lock states), graceful
  SIGTERM stop, daily log rotation (keep 2), `start|stop|status|once|report|verify|config|selftest`;
  `start` polls for the pidfile up to 5 s instead of one fixed 500 ms wait,
  so a slow cold start is not misreported as a failure. `status` shows
  per-target availability over the last hour (reference-only gateways show
  `ref-only` instead of a percentage).
- Deployment: systemd unit (dynamic user), Dockerfile (scratch + static musl
  binary), GitHub Actions CI (fmt + clippy + test, ubuntu + macOS) and
  release matrix (4 static binaries + SHA-256 checksums).
- Rebranded from `netproof` during build; see [REBRAND.md](REBRAND.md).
