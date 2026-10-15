# Deep Diagnosis: netproof — Project State + Roadmap Technical Audit

- **Date:** 2026-10-15 (day 0)
- **Scope:** the netproof "Rust project" as it exists on this machine — 4 planning docs, 0 lines of code — plus live re-verification of every load-bearing external claim, plus a line-level technical audit of [netproof-build-roadmap.md](netproof-build-roadmap.md)
- **Inputs:** [rust-small-project-deep-research.md](rust-small-project-deep-research.md) → [rust-small-project-deep-analysis.md](rust-small-project-deep-analysis.md) → [rust-small-project-final-verdict.md](rust-small-project-final-verdict.md) → [netproof-build-roadmap.md](netproof-build-roadmap.md)
- **Status:** Complete — diagnosis + corrected day-0 checklist

---

## 0. Executive Diagnosis

1. **The project is a decision, not yet a build.** Four decision-complete documents exist; zero code, zero repo, zero CI, zero crate. The gap between "verdict: GO" and "first commit" is entirely unstarted.
2. **The market claims still hold (re-verified live today):** `netproof` crate name **free** (crates.io 404); netband shipped **v1.0.0 (2026-09-18)** but it is **bug-fixes + benchmarks only** — no per-layer attribution, no outage windows, no HTML report, no DNS probe; internet-monitor **still stalled** (last release 2026-06-01, last feature commit 2026-06-01, latest push = FUNDING.yml). **The evidence wedge is open.**
3. **One hard blocker on this machine: no Rust toolchain** (`cargo`/`rustc` not installed). Day 0 cannot start until rustup runs.
4. **One real bug in the roadmap's verbatim `Cargo.toml`:** `[target.'cfg(feature = "icmp")'.dependencies]` — `feature` is not a valid target cfg; cargo rejects it. Canonical fix: `ping = { version = "0.4", optional = true }` + `icmp = ["dep:ping"]` (the feature line already says `dep:ping`, so the author half-fixed it and left the wrong table behind).
5. **One design weakness that can silently kill a differentiator:** the gateway probe is TCP **443 only** (N6). A large fraction of home/office routers do not listen on 443 → `location_class` degrades to `unknown` for every outage → the WiFi-vs-upstream feature (a named differentiator vs netband) is dead on arrival in a meaningful share of deployments. Fix: 443 → 80 fallback, or configurable port list.
6. **One scheduling gap:** the 24h unattended soak (acceptance criterion #2, §9 prerequisite) is **owned by no day** in the day-by-day plan. In a 5-calendar-day budget it must start by end of Day 3 or it can't finish before publish. The plan never says when it starts.
7. **Verdict: the plan is buildable and the GO stands.** Fix the manifest bug (T2), decide the gateway-port question (T3), schedule the soak (T7), install the toolchain (T1). Nothing else blocks day 0.

---

## 1. State of Record (what exists, verified 2026-10-15)

| Artifact | Location | State |
|----------|----------|-------|
| Research (panel + pain evidence + winner) | [rust-small-project-deep-research.md](rust-small-project-deep-research.md) | Final |
| Deep analysis (MECE pillars A–E, CONDITIONAL GO) | [rust-small-project-deep-analysis.md](rust-small-project-deep-analysis.md) | Final |
| Final verdict (unconditional GO, gates, wedge) | [rust-small-project-final-verdict.md](rust-small-project-final-verdict.md) | Final |
| Build roadmap (decision-complete, verbatim artifacts) | [netproof-build-roadmap.md](netproof-build-roadmap.md) | Final — **never executed** |
| Crate source / git repo / CI / crates.io account | — | **DOES NOT EXIST** (searched `~` to depth 4; no `Cargo.toml`, no `netproof` dir anywhere) |
| Rust toolchain on build machine (this Mac) | — | **NOT INSTALLED** (`cargo --version` → exit 127) |
| 24h soak Linux box | — | Unidentified; this Mac covers the macOS half of §9 item 5 |

**Diagnosis:** the organization of the project is 100% in the planning layer and 0% in the execution layer. Every document is internally consistent and each supersedes the last correctly (research → analysis corrected the gap-scan scope → verdict corrected the condition + added netband → roadmap locked deltas D1–D5 + N1–N8). No document contradicts a later one. The failure mode is not analysis — it is **inertia**: the window has a clock (netband is in v1.x stabilization, its obvious next step is the evidence layer) and the clock has been running since 2026-10-15 with no code written.

---

## 2. Live Re-Verification (2026-10-15, day 0)

The verdict's Appendix A scan was designed for T-0 (day 5). Running it at day 0 is free and de-risks committing 5 days.

| # | Claim under test | Live result | Verdict |
|---|------------------|-------------|---------|
| 1 | `netproof` crate name free | `GET crates.io/api/v1/crates/netproof` → `404 {"detail":"crate `netproof` does not exist"}` | **HOLDS** |
| 2 | netband lacks attribution / outage windows / HTML report | README (fetched, valid): "Ping, packet loss, and download/upload tests from your terminal… CSV record you can compare later. Built for Linux." Grep for `attribut|report|html|outage|window|dns|layer` → **zero feature hits** (only "report a problem" in the issues link) | **HOLDS** |
| 3 | netband v1.0.0 didn't close the wedge | v1.0.0 release notes (2026-09-18): provider_id fix, conf-location fix, missing-interface fix, benchmarks. **No evidence-layer features.** Prior releases v0.3.0–v0.6.0 (Sep 1–6) were the ramp; v1.0.0 is the stabilization tag | **HOLDS — wedge open** |
| 4 | internet-monitor stalled | Releases: v0.10.1 (2026-06-01) latest. Commits: FUNDING.yml (2026-09-17), dependabot (2026-08-16), x/crypto, x/net bumps. No headless/CLI item | **HOLDS — stalled** |
| 5 | Build machine ready | `cargo`/`rustc`: **not installed** | **FAILS — blocker T1** |

**Reading of netband's trajectory:** five releases in 18 days (Aug 31 → Sep 18) then a v1.0.0 stabilization tag. That is a builder who has finished the MVP and is now hardening. The evidence layer (attribution + report) is the next obvious product step — the window is open but the person standing in it is one PR away. **This is the single most important fact in the diagnosis: the 5-day budget is not a plan, it is a deadline.**

---

## 3. Document-Chain Consistency Audit

| Check | Result |
|-------|--------|
| Supersession chain (each doc cites and corrects the prior) | Clean. Research's "Rust gap verified (only Python tool found)" is explicitly corrected by analysis §A.3 (internet-monitor missed) and re-scoped by verdict §2.3 (netband narrows wedge to evidence layer). Roadmap inherits the final scoping verbatim in §1.1. |
| Deltas D1–D5 propagated to roadmap | All present: `--once` (§4.1), resolver provenance (§1.2 N2, §4.4 detail format, HTML header), systemd+Dockerfile (§11.3/11.4), D4 copy (§11.7), non-goals (§1.1). |
| Gates T-0/T+30/T+90 propagated | Present in roadmap §8, verbatim. T-0 script duplicated verbatim in §11.8. |
| LOC budget | Verdict "~2k" vs roadmap "2,470 + ~600 tests" — consistent (roadmap is the finer number). |
| Stale references | One: verdict evidence log item 10 records netband "updated 2026-09-18, 120 downloads" — today's query returns the same values plus the v1.0.0 tag the verdict didn't name. No decision impact; the tag only confirms the stabilization reading in §2.3. |
| Scoring integrity | 7-axis matrix (adds competitive occupancy) is applied in verdict §6 and never contradicted. |

**No contradictions found.** The document chain is the strongest part of the project.

---

## 4. Technical Audit of the Roadmap (findings T1–T12)

Severity: **BLOCKER** (stops day 0) · **BUG** (breaks build/publish) · **WEAK** (degrades a differentiator) · **WART** (UX/doc fix) · **NOTE** (cosmetic).

### T1 — BLOCKER: no Rust toolchain on the build machine
`cargo`/`rustc` absent (exit 127). §9 item 1 ("Rust stable toolchain + musl targets") is unmet on the only machine that exists for this project.
**Fix (day 0, ~5 min):** `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y` then `rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl`. (musl cross-builds from macOS are optional — CI builds the Linux targets; the local Mac only needs the native target for dev + the macOS release build.)

### T2 — BUG: `Cargo.toml` feature-gated target table is invalid
Roadmap §11.1 ships:
```toml
[features]
icmp = ["dep:ping"]
...
[target.'cfg(feature = "icmp")'.dependencies]
ping = "0.4"
```
`feature` is not a valid predicate in a target cfg specifier — cargo rejects the manifest (the `dep:ping` syntax in the feature line proves the intended mechanism was optional-dependency; the target table is a leftover).
**Fix:** delete the `[target.'cfg(feature = "icmp")'.dependencies]` table; put `ping = { version = "0.4", optional = true }` in `[dependencies]`. The `icmp = ["dep:ping"]` line then works as written. (Verify empirically at scaffold — no toolchain existed to test this while writing the roadmap; the fix is the canonical form regardless.)
**Consequence if unfixed:** day 0 exit criterion ("CI green on an empty-but-compiling tree") fails at `cargo check`.

### T3 — WEAK: gateway probe is TCP 443 only → location_class may be permanently `unknown`
N6: gateway = auto-detected default gateway, probed on **TCP 443**, 2s timeout; no answer → `location_class = unknown`. Many consumer/office routers do not expose 443 (80 is the common admin port; some expose neither). In those deployments — plausibly the majority of the home-user audience — the WiFi-vs-upstream classification, a named differentiator against netband, never fires. The roadmap's "never a wrong claim" stance is correct, but a feature that outputs `unknown` 100% of the time is not a feature.
**Fix (pick one, day 1):**
- (a) Probe 443, then fall back to 80 on connect-refused/timeout; record which port answered in `detail`. ~20 LOC. **Recommended.**
- (b) `endpoint = "auto:443,80"` config syntax (port list).
- (c) Accept the degradation; document it prominently in README FAQ and in the report header ("gateway port 443 unreachable — location classification unavailable").
**Do not** silently keep 443-only: the report would show `unknown` in the outage table and the user's ISP argument loses its strongest clause without knowing why.

### T4 — WART: `status`/`stop` don't work cross-user under the systemd deployment
§11.3 unit runs `DynamicUser=true` with `StateDirectory=netproof` → DB/lock/logs under `/var/lib/netproof/` owned by an ephemeral user. A normal user running `netproof status` (reads DB) or `netproof stop` (SIGTERM to the daemon) will hit permission denials. The pidfile path only covers the user-`start` deployment.
**Fix (day 4, doc + one behavior):** README states the systemd-path commands are `systemctl status netproof` / `systemctl stop netproof`; `status` detects "DB not readable, but pidfile/lock indicates running" and degrades gracefully instead of erroring. ~15 LOC + copy.

### T5 — WART: `status` "uptime" has no defined source of truth
§4.2 shows `uptime 3d 4h`. No field stores daemon start time (pidfile mtime is wrong after a restart reuses the file; WAL doesn't help). **Fix:** write `meta.daemon_started_at` at `run --foreground` start; `status` reads it. 5 LOC.

### T6 — NOTE: `edition = "2021"` in a 2026 crate
Edition 2024 has been stable since Rust 1.85 (Feb 2025). No functional impact; updating is one line and avoids looking dated in a README that leads with engineering quality. Optional.

### T7 — WEAK (scheduling): the 24h soak is owned by no day
§0 acceptance #2 requires "24h unattended: no crash, retention enforced, report renders." §9 requires two live boxes "for the 24h soak." But the day plan (Day 0–5) never schedules it: Day 4 ends with manual systemd/Docker checks, Day 5 is publish. In a 5-calendar-day budget, a 24h soak that starts end-of-Day-4 finishes end-of-Day-5 — publish day — leaving zero margin, and any soak failure (crash on hour 19) eats the publish slot.
**Fix (re-baseline the plan):** start the soak **at Day 3 exit** (selftest PASS is the trigger — the daemon is stable enough to run unattended). It then completes during Day 4 while daemon/deployment work happens on a *second* config/box, or on the same box in a separate data dir. Publish (Day 5) is gated on T-0 scan + CI + selftest (as §8 already defines) — the soak result is recorded in `PUBLISH-GATE.md` as acceptance evidence, and if it fails, v0.1.0 ships with the fix as v0.1.1 only if the failure is non-evidence-integrity (a crash is a crash; that one blocks).

### T8 — NOTE: dep count "13" vs actual 16+1
§2.4 says "13, all static"; §11.1 lists 16 mandatory + 1 optional. Cosmetic; the "all static, no CGO" claim is what matters and it holds (rusqlite/bundled compiles C via `cc` at build time — build-time only, binary stays static).

### T9 — NOTE: `start` liveness check has a 500ms blind spot
A daemon that crashes at 501ms returns "started" to the user. `status` catches it later; acceptable for v1. Document in README FAQ ("if `start` succeeded but `status` shows not running, check the log file").

### T10 — NOTE: `cargo test --all-features` in CI builds the `ping` crate on macOS
Correct behavior (tests the feature), adds ~1–2 min to macOS CI. No action.

### T11 — NOTE: Dockerfile CA-cert copy is redundant but harmless
`reqwest` + `rustls-tls` uses `webpki-roots` (embedded roots) by default, so `COPY /etc/ssl/certs` in the scratch image is not needed for TLS verification. Keep it — it costs 2 MB and future-proofs a switch to native roots. No action.

### T12 — NOTE: DNS golden-fixture test is the right call
Hand-rolled TCP/53 A-query (N2) is ~150 LOC of the most auditable code in the project; the exact-byte golden fixtures (Day 1 test plan) are the correct test shape. The fixed query ID + RD=1 is standard. No action.

---

## 5. Updated Risk Register (post-diagnosis)

| # | Risk | P | Impact | Delta vs verdict |
|---|------|---|--------|------------------|
| R1 | netband ships attribution + report before our publish | med | high | **UP** — v1.0.0 stabilization means the feature work is next; window is now measured in weeks, not months |
| R2 | 5-day budget slips (renderer, M7) | med | med | unchanged; contingency (table-only v0.1.0) pre-committed — keep it |
| R3 | 24h soak fails late (crash, retention bug) | low-med | med | **NEW** (T7) — mitigated by starting soak at Day 3 |
| R4 | Toolchain/environment friction on day 0 | low | low | **NEW** (T1) — 10 min to clear |
| R5 | Manifest bug (T2) wastes a day | low | low | **NEW** — fix in scaffold, not discovered in CI |
| R6 | location_class dead-on-arrival (T3) | med | med | **NEW** — differentiator erosion, not a build failure |
| R7 | T+30 weak adoption | med | med | unchanged; gate defined |
| R8 | Name taken between now and publish | very low | high | re-check at T-0 (scripted) |

---

## 6. Verdict

**The plan survives contact with the codebase that doesn't exist yet.** The document chain is consistent, the market claims re-verify clean today, and the roadmap is decision-complete to the point of shipping verbatim `Cargo.toml`, SQL, systemd unit, Dockerfile, and CI YAML. The four real defects are: a missing toolchain (10 min), a manifest bug (2 min to fix in the scaffold), a gateway-port design gap (20 LOC), and an unscheduled soak (plan re-baseline). None of them touches the wedge, the spec, or the gates.

**The GO stands. The only risk that now matters is the clock:** netband is in stabilization, and its next PR is our kill criterion. Execute days 0–5 as a deadline, not a plan.

---

## 7. Corrected Day-0 Checklist (supersedes roadmap §5 Day 0)

1. Install rustup + stable toolchain (T1). ~5 min.
2. `cargo new netproof --bin` — in a new git repo (create the GitHub repo first; CI needs it).
3. Apply §11.1 `Cargo.toml` **with the T2 fix** (`ping` as optional dep, no feature-target table) and `edition = "2024"` (T6, optional).
4. `LICENSE` (MIT), `.gitignore` (`/target`, `*.db*`), `CHANGELOG.md` stub.
5. Module skeleton per §2.1 — `todo!()` bodies, `cargo check` green.
6. CI skeleton per §11.5 (fmt + clippy + test, ubuntu + macOS); push; verify CI runs.
7. **Exit criterion:** CI green on an empty-but-compiling tree, manifest accepted by cargo.

Then: Day 1 (probes + `--once`, **with T3 fix (a)**: 443→80 gateway fallback), Day 2 (detect + store), Day 3 (report + selftest; **start 24h soak at exit** — T7), Day 4 (daemon + systemd + Docker; **T4/T5 fixes**), Day 5 (T-0 gate → publish).

---

## 8. Master Map

```
DEEP DIAGNOSIS — netproof (day 0)
├── 0 EXECUTIVE ..................... 7 findings; GO stands; clock is the risk
├── 1 STATE OF RECORD ............... 4 docs final, 0 code, 0 repo, no toolchain
├── 2 LIVE RE-VERIFICATION .......... name free; netband v1.0.0 = fixes only;
│                                     internet-monitor stalled; wedge OPEN
├── 3 DOCUMENT-CHAIN AUDIT .......... consistent; no contradictions; one stale datum
├── 4 TECHNICAL AUDIT (T1–T12)
│   ├── T1 BLOCKER .................. no Rust toolchain on build machine
│   ├── T2 BUG ........................ Cargo.toml feature-target table invalid → optional dep
│   ├── T3 WEAK ..................... gateway 443-only → location_class unknown; add 80 fallback
│   ├── T4 WART ..................... status/stop under DynamicUser systemd → doc + graceful degrade
│   ├── T5 WART ..................... uptime source undefined → meta.daemon_started_at
│   ├── T6 NOTE ..................... edition 2021 → 2024
│   ├── T7 WEAK ..................... 24h soak owned by no day → start at Day 3 exit
│   └── T8–T12 NOTE ................. dep count, 500ms blind spot, CI time, CA certs, DNS fixtures
├── 5 RISK REGISTER (UPDATED) ....... R1 up (netband clock); R3–R6 new
├── 6 VERDICT ....................... buildable; GO stands; execute as deadline
└── 7 CORRECTED DAY-0 CHECKLIST ..... 7 steps, supersedes roadmap §5 Day 0
```
