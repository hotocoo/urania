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

1. **GitHub repo** — create `hotocoo/urania` (or preferred org) and push;
   the CI + release workflows are ready.
2. **Publish channel** — prebuilt static binaries via GitHub Releases are the
   primary channel (no account needed beyond GitHub). `cargo publish` to
   crates.io is optional for this product (it's a binary, not a library);
   the name `urania` is free if desired.
3. **24h soak** (roadmap T7) — start the soak before tagging v0.1.0.

## 4. Gate status

- [x] name free on crates.io
- [x] wedge open (re-verified)
- [ ] 24h soak (T7)
- [ ] trademark search (before commercial distribution)
- [ ] repo + first release
