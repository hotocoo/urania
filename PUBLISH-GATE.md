# Publish gate (T-0)

The roadmap's T-0 gate: re-verify name availability and the evidence wedge
before publishing. Performed 2026-10-15.

## 1. Name availability

- crates.io `urania`: **free** (HTTP 404, 2026-10-15).
- crates.io `netproof` (original plan): free per the planning docs (2026-10-15).
- Web conflicts for `urania`: two small libraries in other languages
  (Clojure, Scala); no network tooling. See REBRAND.md for the full scan.
- ⚠ No trademark search performed. Required before commercial distribution.

## 2. Evidence wedge (competitor scan)

- **netband** (gregl83, Rust, v1.0.0 2026-09-18): bug-fixes only since
  release — no outage windows, no per-layer attribution, no HTML report, no
  DNS probe. The wedge (attachable multi-layer evidence) remains open.
- **internet-monitor** (Go): stalled (last feature commit 2026-06-01), no
  LICENSE file. Not a credible incumbent.
- No other headless, zero-permission, evidence-grade outage logger found.

**Conclusion: the wedge is open; publishing is not blocked by competition.**

## 3. Blockers / decisions for the owner

1. **GitHub repo** — done 2026-10-15: [hotocoo/urania](https://github.com/hotocoo/urania)
   created, `main` pushed, CI green (fmt + clippy + test, ubuntu + macOS,
   plus a static-musl job asserting `file` reports statically linked).
2. **Publish channel** — prebuilt static binaries via GitHub Releases are the
   primary channel (no account needed beyond GitHub). `cargo publish` to
   crates.io is optional for this product (it's a binary, not a library);
   the name `urania` is free if desired.
3. **24h soak** (roadmap T7) — start the soak before tagging v0.1.0.

## 4. Gate status

- [x] name free on crates.io
- [x] wedge open (re-verified)
- [x] icmp feature live-verified (2026-10-15: 9.0ms RTT to 1.1.1.1, unprivileged macOS)
- [x] 24h soak (T7) — started 2026-10-15 21:19:55 +08 (pid 70840; replaced
      after 7 min by pid 81779, running continuously since 21:26:38).
      Verified 2026-10-19: 3d 11h, 300,600 probes, three full days at
      exactly 86,400 rows/day (zero missed intervals), 5/6 targets at
      99.9–100% availability, gateway reference-only as designed (never
      answered TCP → no outage claimed; its single db window is the closed
      `reference-only: …` correction), no true clock anomalies (the dns
      drift flags were response-duration jitter, fixed in this release).
- [ ] trademark search (before commercial distribution)
- [x] repo (2026-10-15: `hotocoo/urania` created, `main` pushed, CI green)
- [ ] first release (tag `v0.1.0` after the soak passes → 4 static binaries + SHA256SUMS via the release workflow)
