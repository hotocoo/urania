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
