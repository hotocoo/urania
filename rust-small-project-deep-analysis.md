# Deep Analysis: Rust Small-Project Findings (netproof Recommendation)

- **Date:** 2026-10-15
- **Input:** [rust-small-project-deep-research.md](rust-small-project-deep-research.md) (2026-10-15, "Build netproof" verdict)
- **Method:** MECE four-step deconstruction + live re-verification of the gap claim (crates.io API, GitHub API, web search)
- **Status:** Complete — conditional verdict

---

## 0. Executive Verdict

1. The findings are **structurally sound** (panel, triangulation, scoring, red-team) but contain **one fatal blind spot**: gap verification scanned **Rust incumbents only**, not cross-language incumbents.
2. **New evidence (verified today):** [internet-monitor](https://github.com/FutureSolutionDev/internet-monitor) — Go, MIT, 63★, created **2026-05-11** (5 months *before* the research), last push 2026-09-17, active CI — already ships the entire netproof concept: 24/7 background TCP/HTTP/DNS probes, per-layer outage attribution ("DNS failed while the link was up"), monthly **PDF + CSV reports for ISP tickets**, optional ICMP LAN-vs-ISP diagnosis, speed tests, webhooks, cross-platform binaries, "100% local, no telemetry". Its tagline: *"Document your outages. Win the dispute with your ISP."*
3. The findings' core differentiator — *"unique output (evidence artifact) is the adoption hook"* — is **refuted at market level**. [PingPlotter](https://www.pingplotter.com/) (commercial, years old) and [v-ping](https://vtoollab.com/tools/v-ping/) (Windows) do the same value proposition.
4. The **Rust-level gap holds**: no Rust implementation found; the `netproof` crate name is **free** (crates.io API: `crate netproof does not exist`).
5. **Verdict: CONDITIONAL GO** — only under repositioning as the **headless, terminal-first, Rust single binary for servers, Docker, CI, and SSH workflows** (internet-monitor is desktop-tray/dashboard-oriented). **NO-GO** if the goal is a broad consumer app. `envdrift` rises in relative rank.

---

## 1. STEP 1 — Boundary Definition and Scope Isolation

**Inside the universe (this analysis):**
- Validity of the research artifact (method, evidence, scoring).
- The `netproof` product space: requirements, architecture, platform, testing, distribution, effort.
- Risk register for the recommendation.
- Decision: go/no-go, sequencing, kill criteria.

**Strictly outside:**
- Detailed build plans for `diskdoctor` / `envdrift` (ranked only, not decomposed).
- Implementation, branding, licensing beyond name availability, and any non-Rust build.

**Baseline terms (anti-drift definitions):**
- **Market gap** = no existing single-binary tool (any language) well-solves the pain — the findings' *own* definition of "pain point".
- **Rust gap** = no Rust implementation exists.
- The findings **conflate these two**. That conflation is the root cause of the missed incumbent (see A.3).
- **Table stakes** = features the new evidence shows users now expect in this category (per-layer attribution, LAN-vs-ISP, PDF/CSV export).

---

## 2. STEP 2 — Highest-Level Pillar Separation (Mutually Exclusive)

| Pillar | Domain | Question it answers |
|--------|--------|---------------------|
| **A** | Findings validity | Is the research artifact correct? |
| **B** | Product definition | What must netproof do? |
| **C** | Engineering delivery | How is it built and shipped? |
| **D** | Risk register | What can go wrong, with what probability/impact? |
| **E** | Decision | What do we do, in what order, when do we stop? |

**Mutual-exclusivity proof:**
- A judges the *past* (the artifact); B/C/D/E judge the *future* (the build). No variable is both a claim about the research and a claim about the product.
- Same subject, different attributes, exactly one home each: e.g. ICMP appears in **B** as a *requirement* (capability-gated, off by default) and in **D** as a *risk* (firewall probability × impact). The issue is counted once; B states what the system must do, D states how it can fail. No double counting.
- Competitive facts live in **D.1** (risk: occupancy), not in A (which only audits whether the research *caught* them) — keeping A about the artifact's quality, D about the world's state.

**Collective-exhaustiveness audit:** any aspect of "deeply analyze the findings" is either (i) a claim about the research → A, (ii) a claim about the product → B, (iii) a claim about the build → C, (iv) a claim about failure → D, (v) a claim about action → E. No residual category exists.

---

## 3. STEP 3 — Deep Sub-Component Drill-Down

### Pillar A — Findings Validity

#### A.1 Methodology
- **Functional definition:** panel composition + triangulation + gap verification + scored synthesis.
- **Assessment:** sound in design. Five distinct expert roles, three source types triangulated, red-team pass, explicit disagreement resolution.
- **Vulnerability:** gap verification was scoped to "what's already built in Rust" (E1's mandate) — a Rust-ecosystem question, not a market question. The method's strongest step was the narrowest.

#### A.2 Evidence quality
- **Functional definition:** do the cited sources actually support the claims?
- **Assessment:** all 8 pain-source links are real and on-topic; freshness adequate (2020–2026); one acknowledged fetch failure (r/rust thread body) — correctly disclosed, correctly judged non-material.
- **Vulnerability:** evidence for *pain* is strong; evidence for *emptiness of the space* is the weak half, and it failed (A.3).

#### A.3 Gap verification — **FAILED**
- **Functional definition:** confirm no incumbent well-solves the pain.
- **Findings' result:** "only a Python tool found" (CryPing).
- **Re-verification (2026-10-15):**
  | Incumbent | Language | Age | State | Overlap with netproof |
  |-----------|----------|-----|-------|----------------------|
  | [internet-monitor](https://github.com/FutureSolutionDev/internet-monitor) | Go | 5 months (2026-05-11) | Active (push 2026-09-17, CI, 63★, MIT) | **Near-total**: probes, per-layer attribution, PDF/CSV ISP reports, LAN-vs-ISP, speed tests, webhooks |
  | [PingPlotter](https://www.pingplotter.com/) | commercial | years | Mature, paid | Latency/loss/jitter timeline, "share the evidence" |
  | [v-ping](https://vtoollab.com/tools/v-ping/) | Windows | n/a | Live | Timestamped ping logging with persistence |
  | CryPing | Python | n/a | Small | TCP-ping dropouts (already in findings) |
- **Consequence:** by the findings' own definition of pain ("not solved well by an existing single-binary tool"), the pain **is** solved — the market gap is false; only the Rust gap survives.
- **Vulnerability index:** **critical** — this single miss inverts the "unique adoption hook" argument that decided the winner.

#### A.4 Scoring integrity
- **Functional definition:** does the 6-axis matrix measure what decides the choice?
- **Assessment:** unweighted 6 axes (Correctness, Completeness, Usefulness, Safety, Clarity, Efficiency). Structural blind spot: **no "competitive occupancy" axis** — the dimension on which netproof now fails was never scored. All three candidates scored Correctness=5 with no justification shown.
- **Vulnerability index:** high — the matrix *cannot* have caught the A.3 failure even if the evidence had been present.

#### A.5 Assumption audit
- "Small" = 1–5k LOC: netproof MVP fits (~2k LOC cut line, §C.5).
- New constraint from evidence: internet-monitor's feature bar (speed tests, webhooks, dashboard, auto-update, bilingual UI) **resets user expectations** for this category. A thinner Rust tool must not be judged against it feature-by-feature; it must be judged on a different axis (headless/terminal/Rust distribution).

### Pillar B — Product Definition (netproof)

#### B.1 Probe layer
- **B.1.1 DNS probe** — target domains; success = resolution within timeout. *Inputs:* target list, resolver choice, timeout. *Dependencies:* B.3 (storage), D.2.2 (resolver caching invalidates the measurement — probe TCP/53 and multiple resolvers; state resolver in report).
- **B.1.2 TCP probe** — default targets `1.1.1.1:53` / `8.8.8.8:53` (TCP, avoids UDP-53 ISP blocking, D.2.3); success = handshake.
- **B.1.3 HTTP probe** — default 204 endpoints (e.g. `connectivitycheck.gstatic.com`); success = 2xx/204 within timeout.
- **B.1.4 ICMP probe** — **optional, off by default**, capability-checked per platform (Linux CAP_NET_RAW/root; macOS firewall prompt; Windows admin). *Vulnerability:* permission friction (D.2.1).
- **Exhaustiveness:** every probe netproof can run is one of DNS/TCP/HTTP/ICMP; every probe has target, interval, timeout, success criterion. No probe type omitted.

#### B.2 Detection layer
- **B.2.1 Outage-window detection** — consecutive-failure threshold (table stakes: internet-monitor uses `fail_threshold=3`). *Inputs:* probe stream. *Outputs:* outage start/end.
- **B.2.2 Per-layer cause attribution** — "DNS failed while link was up". **No longer a differentiator — table stakes** (internet-monitor's headline feature).
- **B.2.3 Flap detection** — distinguish repeated short outages from one sustained outage (reporting clarity).
- **B.2.4 WiFi-vs-upstream classification** — gateway probe vs external probe (table stakes: internet-monitor's LAN-vs-ISP).
- **Exhaustiveness:** every failure the tool can observe is classified as (probe-level failure → window → attributed layer → location class). No unclassified failure mode.

#### B.3 Storage layer
- **B.3.1 SQLite schema** — `probes(ts, target, type, ok, latency_ms, detail)`, `outages(start, end, layers, cause)`, `meta(schema_version, clock_note)`. *Inputs:* probe stream. *Dependencies:* B.2 (windows), B.4 (report queries).
- **B.3.2 Retention cap** — default 90 days, configurable; prune + vacuum on schedule. *Vulnerability:* unbounded growth (D.2.5) — mitigated by cap.
- **B.3.3 Clock integrity** — store wall clock + monotonic delta; report states NTP status. *Vulnerability:* drift undermines the evidence's credibility (D.3).

#### B.4 Reporting layer
- **B.4.1 HTML report** — per-target latency sparklines, outage table with per-layer attribution, availability %, clock note, target list. Single self-contained file (the attachable artifact).
- **B.4.2 CSV export** — raw probe rows for spreadsheet/analysis use.
- **B.4.3 Evidence credibility** — multi-target defaults (never a single 8.8.8.8 — single-target bias is the #1 way such evidence gets dismissed), optional SHA-256 manifest over the CSV.
- **Exhaustiveness:** report contains every datum needed for an ISP/IT ticket: when (windows), how bad (latency/loss/jitter), where (layer + location), measured how (targets, clock, tool version).

#### B.5 Process layer
- **B.5.1 CLI surface** — `netproof start | stop | status | report | selftest`.
- **B.5.2 Single-instance lock; no local HTTP server** — **the differentiator** vs internet-monitor (which runs a dashboard on `localhost:8765`, needs a port, a browser, WebKitGTK on Linux). Headless = safe on locked-down servers.
- **B.5.3 No root by default; no auto-start at v1** (opt-in later).

#### B.6 Non-goals (v1) — explicit, to prevent scope creep
- No speed tests, no webhooks, no GUI/dashboard, no Windows, no auto-update, no plugin API. All exist in internet-monitor; competing there is a lost fight. (Each re-evaluated at v2 against D.1.)

### Pillar C — Engineering Delivery

#### C.1 Architecture
- **Functional definition:** single static binary; `tokio` (async probe scheduler), `rusqlite` (storage), `clap` (CLI).
- **Module sketch:** `netproof/{probe, detect, store, report, daemon, cli}`.
- **Dependencies:** none required beyond the three crates + `chrono`/`serde`-class utilities. No server, no CGO.

#### C.2 Platform matrix
- v1: **Linux + macOS** (the pain's home turf: servers + dev machines). Windows v2. ICMP off by default on all platforms.

#### C.3 Testing strategy
- `--selftest`: prove probe→store→report round-trip against known-good targets *before* daemon mode (from findings' next steps — kept).
- Offline unit fixtures: synthetic probe streams exercising threshold, flap, and per-layer attribution logic (detection must be testable without network).
- Integration: start→probe→stop→report in CI against a local dummy target.

#### C.4 Distribution
- `cargo install netproof` (crates.io — name verified free); GitHub Releases static binaries; no installer, no service manager required at v1.

#### C.5 Effort (MVP cut line ≈ 2k LOC)
| Step | Content | Effort |
|------|---------|--------|
| 1 | Scaffold + TCP/HTTP/DNS probe loop | ~1 day |
| 2 | Detection (windows, attribution, flap) + SQLite | ~1 day |
| 3 | HTML/CSV report + `--selftest` | ~1 day |
| 4 | Daemon start/stop/status + single-instance | ~1 day |
| 5 | Publish + positioning copy | ~1 day |
- Optional later: ICMP (+2–3 days), Windows (+2 days), auto-start (+1 day).

### Pillar D — Risk Register

#### D.1 Competitive — probability: medium · impact: **high (consumer position) / low (headless position)**
- internet-monitor: active, MIT, 63★, roadmap items (per-target latency history, plugin API) show momentum but also gaps (no Windows ARM64, no headless mode, dashboard-centric).
- PingPlotter: entrenched commercial incumbent in the same "share the evidence" framing.
- **Mitigation:** position as *"the headless, terminal-first evidence logger — for servers, Docker, CI, and SSH workflows"*; ship via crates.io; never compete on dashboard/webhooks/speed tests.

#### D.2 Technical
- **D.2.1 ICMP permissions** (root/firewall) → off by default; capability check.
- **D.2.2 DNS resolver caching** → probe TCP/53, multiple resolvers; state resolver in report.
- **D.2.3 ISP UDP-53 blocking** → TCP fallback targets.
- **D.2.4 Clock drift** → monotonic delta + NTP note in report.
- **D.2.5 SQLite growth** → retention cap + vacuum.

#### D.3 Evidence integrity — probability: low · impact: high
- Single-target bias → multi-target defaults (B.4.3).
- Tamper suspicion → optional SHA-256 manifest.
- Clock disputes → clock note (B.3.3).

#### D.4 Adoption — probability: medium · impact: medium
- Discoverability + naming: `netproof` free, on-message. Messaging must name the differentiator (headless/Rust/no-server), not repeat the occupied "prove your ISP wrong" framing alone.

### Pillar E — Decision

#### E.1 Verdict
- **CONDITIONAL GO.** GO iff positioned headless/terminal-first (server/CI/SSH niche). NO-GO for broad consumer positioning (market occupied: internet-monitor + PingPlotter).
- **Revised ranking:** netproof (repositioned) > envdrift > diskdoctor. envdrift rises: its only competitor (dotenv-linter) is narrow (single-file linting), and the cross-project drift wedge is untouched by today's evidence.

#### E.2 Sequencing
1. Day 1: scaffold + TCP/HTTP/DNS probe loop.
2. Day 2: detection + SQLite.
3. Day 3: report + `--selftest`.
4. Day 4: daemon lifecycle.
5. Day 5: publish + positioning.

#### E.3 Kill criteria (stop if any true)
- internet-monitor ships a headless/CLI mode (wedge closed).
- Target audience turns out to be desktop-consumer (wrong wedge).
- Re-verification at publish time finds another Rust probe-logger crate (gap re-closed).

---

## 4. STEP 4 — Cross-Pillar Interaction & Trade-Off Dynamics

| Lever (pillar) | Raises | Costs (other pillar) |
|----------------|--------|----------------------|
| Probe frequency ↑ (B.1) | Evidence granularity (B.4) | SQLite growth + vacuum load (B.3/D.2.5), network load, battery (C.2) |
| ICMP on by default (B.1.4) | Diagnostic power (B.2.4) | Root/firewall friction (D.2.1), platform matrix cost (C.2) |
| Report richness ↑ (B.4) | Adoption hook (D.4) | Renderer LOC + binary size (C.1/C.5) |
| Retention ↑ (B.3.2) | Long-term evidence (B.4.3) | Disk + query time (D.2.5) |
| Headless-only (B.5.2) | Sharpens sysadmin wedge (D.1) | Narrows consumer funnel (E.1) |
| No local server (B.5.2) | Simpler, safer, no port conflict (C.1) | No live dashboard — the feature the incumbent *does* have (D.1) |
| Thinner MVP (C.5) | Speed to publish (E.2) | Expectations gap vs incumbent feature bar (A.5) |

**Systemic equilibrium (v1 defaults):** TCP/HTTP/DNS every 5–10 s; ICMP off; 90-day retention; HTML + CSV report; headless CLI; no server; Linux + macOS. This point maximizes evidence value per unit of friction, LOC, and risk.

---

## 5. Master Taxonomy Map

```
DEEP ANALYSIS — netproof recommendation
├── A FINDINGS VALIDITY
│   ├── A.1 Methodology ............... sound design; Rust-only gap scope (root cause)
│   ├── A.2 Evidence quality .......... pain evidence strong; emptiness evidence weak
│   ├── A.3 Gap verification .......... FAILED: internet-monitor (Go) missed
│   ├── A.4 Scoring integrity ......... no "competitive occupancy" axis
│   └── A.5 Assumption audit .......... expectation bar reset by incumbent
├── B PRODUCT DEFINITION
│   ├── B.1 Probes: DNS / TCP / HTTP / ICMP(opt)
│   ├── B.2 Detection: windows / attribution / flap / location
│   ├── B.3 Storage: schema / retention / clock
│   ├── B.4 Report: HTML / CSV / credibility
│   ├── B.5 Process: CLI / no-server / no-root
│   └── B.6 Non-goals v1: speed, webhooks, GUI, Windows
├── C ENGINEERING DELIVERY
│   ├── C.1 Architecture: tokio + rusqlite + clap
│   ├── C.2 Platform: Linux+macOS (v1)
│   ├── C.3 Testing: selftest / fixtures / CI round-trip
│   ├── C.4 Distribution: crates.io + releases
│   └── C.5 Effort: ~2k LOC, 5 days
├── D RISK REGISTER
│   ├── D.1 Competitive: internet-monitor / PingPlotter / v-ping
│   ├── D.2 Technical: ICMP / DNS cache / UDP-53 / clock / growth
│   ├── D.3 Evidence integrity: target bias / tamper / clock
│   └── D.4 Adoption: naming / messaging
└── E DECISION
    ├── E.1 Verdict: CONDITIONAL GO (headless position only)
    ├── E.2 Sequencing: 5-day plan
    └── E.3 Kill criteria: headless incumbent / wrong audience / re-occupied gap
```

---

## 6. Evidence Appendix (new, verified 2026-10-15)

1. [internet-monitor](https://github.com/FutureSolutionDev/internet-monitor) — GitHub API: `language: Go`, `created_at: 2026-05-11T15:33:33Z`, `pushed_at: 2026-09-17T18:34:50Z`, 63 stars, 1 fork, 0 open issues, MIT (README; repo metadata showed `license: null` — badge + README state MIT, verify before relying on it). README feature list: simultaneous TCP/HTTP/DNS checks, per-layer outage logging with timestamps/durations, monthly PDF + CSV/JSONL export, LAN-vs-ISP via optional ICMP, speed tests (Cloudflare endpoints), Discord/Slack/Telegram webhooks, Prometheus `/metrics`, system-tray + dashboard on `localhost:8765`, Windows/macOS/Linux binaries, auto-update.
2. [PingPlotter](https://www.pingplotter.com/) — commercial; "Plot results on an infinite timeline… Share the evidence."
3. [v-ping](https://vtoollab.com/tools/v-ping/) — Windows connectivity logger; "No persistence… missing timestamps" as the stated gap it fills.
4. crates.io API `GET /api/v1/crates/netproof` → `404 {"errors":[{"detail":"crate `netproof` does not exist"}]}` — name available.
5. [trippy](https://crates.io/crates/trippy-tui/0.11.0-rc.2) — Rust live traceroute/ping; confirms live-monitoring space is occupied in Rust (consistent with findings), but is a live tool, not an evidence logger.
