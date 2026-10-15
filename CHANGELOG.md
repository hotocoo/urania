# Changelog

All notable changes to urania are documented here.

## 0.1.0 (unreleased)

- Initial release: headless, zero-permission network outage evidence logger.
- Probes: DNS (hand-rolled A query over TCP/53, resolver recorded per probe),
  TCP, HTTP (rustls, no OpenSSL); ICMP optional behind the `icmp` feature.
- Gateway auto-detect (Linux `ip route`, macOS `route -n get default`) with
  443→80 fallback; imperfection degrades to `location_class = unknown`.
- Detection: outage windows (backdated to first failure), per-layer cause
  attribution, flap detection, WiFi-vs-upstream location class.
- Storage: SQLite (WAL, bundled C source → fully static), 90-day retention,
  clock integrity (wall clock + monotonic delta per probe), restart recovery.
- Report: self-contained HTML (inline SVG sparklines, zero JavaScript, zero
  external requests), deterministic CSV, optional SHA-256 manifest.
- Daemon: single-instance flock + pidfile (no stale-lock states), graceful
  SIGTERM stop, daily log rotation (keep 2), `start|stop|status|once|report|selftest`.
- Deployment: systemd unit (dynamic user), Dockerfile (scratch + static musl
  binary), GitHub Actions CI (fmt + clippy + test, ubuntu + macOS) and
  release matrix (4 static binaries + SHA-256 checksums).
- Rebranded from `netproof` during build; see [REBRAND.md](REBRAND.md).
