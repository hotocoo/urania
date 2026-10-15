# Final Verdict: `netproof` — Small Rust Project Adjudication

- **Date:** 2026-10-15
- **Input:** [rust-small-project-deep-research.md](rust-small-project-deep-research.md) (recommendation) + [rust-small-project-deep-analysis.md](rust-small-project-deep-analysis.md) ("CONDITIONAL GO")
- **Method:** Adjudication of the conditional verdict + full live re-verification of every load-bearing claim (crates.io API, GitHub API, README/docs fetches, vendor site) + meta-audit of the analysis document's own blind spots
- **Status:** Final — **GO (unconditional, on the evidence wedge)**

---

## 0. Executive Verdict

1. **Final verdict: GO — unconditional.** The prior document's "CONDITIONAL GO" misclassified a *strategy choice* (positioning) as a *condition*. Positioning is under the builder's control; it is a specification, not a risk. The product to build is fully specified in §5; the external risks live in pre-committed gates (§7), not in the verdict.
2. **Material new evidence since the last analysis: `netband`** ([github.com/gregl83/netband](https://github.com/gregl83/netband)) — Rust, Apache-2.0, created **2026-08-31** (six weeks ago), active CI, checksum-verified Linux releases. It already ships the **headless half** of the wedge: a systemd service (non-root dynamic user), persistent CSV journals, continuous monitoring. **The wedge is therefore no longer "headless" — it is "evidence":** no-permission TCP/HTTP/DNS probes, per-layer cause attribution, outage-window detection, and an attachable HTML/CSV report. `netband` is a witness with a raw log; `netproof` is a witness with a case file.
3. **Corrections to the prior analysis's evidence:** internet-monitor is **stalled**, not active (last release v0.10.1 on 2026-06-01; no feature commits since — the latest push was a sponsorship link), and its **MIT claim is unsupported** (the README badge links to a `LICENSE` file that does not exist in the repo).
4. **Kill criteria re-run (2026-10-15): none triggered.** `netproof` crate name still free (crates.io API 404). No Rust outage-evidence-logger exists (systematic 7-query-family scan, Appendix A).
5. **Final ranking** (corrected 7-axis matrix, adds the "competitive occupancy" axis the prior analysis found missing but never re-scored): **netproof 32/35 > envdrift 30/35 > diskdoctor 28/35.** Fallback chain: netproof → envdrift → diskdoctor.
6. **Decision rule:** build now (5 days). Pre-publish gate (day 5): re-run the scripted gap scan — if the evidence wedge is occupied, pivot to envdrift. T+30 adoption gate and T+90 wedge-closure gate in §7.

---

## 1. Adjudicating the "CONDITIONAL GO"

The prior document's condition: *"GO iff positioned headless/terminal-first; NO-GO for broad consumer positioning."*

- A condition whose satisfaction depends **only on the decision-maker's own strategy** is not a condition — it is the product spec. "Position headless" is a choice we make on day 1; it cannot "fail to be met" by the world.
- The genuine external risks (incumbent ships headless mode, gap re-occupied, wrong audience) were correctly identified — but they belong in **kill criteria with measurable triggers**, which the prior document only half-defined (its criterion 2, "audience turns out to be desktop-consumer," is not measurable; §7 replaces it).
- **Consequence:** the verdict converts to an **unconditional GO on the specified product** (§5), with all uncertainty pushed into gates (§7). A conditional verdict that never commits is the worst outcome: it consumes the window (netband's roadmap is live) without shipping.

---

## 2. Live Re-Verification (2026-10-15)

### 2.1 Kill criteria

| # | Criterion | Test (live today) | Result |
|---|-----------|-------------------|--------|
| KC1 | Incumbent ships headless/CLI mode | internet-monitor README + repo file list + roadmap + commit history | **Not triggered.** Two modes only: system tray + standalone window; dashboard on `localhost:8765`; Linux build is tray-only. Roadmap items: Windows ARM64, mobile-responsive dashboard — no headless item. |
| KC2 | Audience is desktop-consumer | Not measurable pre-build | **Replaced** with measurable T+30 rule (§7). |
| KC3 | Rust gap re-occupied | Systematic crates.io scan, 7 query families (Appendix A) | **Not triggered** — no Rust outage-evidence-logger. **Warning:** `netband` occupies the adjacent headless-tester niche (new since last analysis, §2.3). |
| — | Name availability | `GET crates.io/api/v1/crates/netproof` → `404 {"detail":"crate netproof does not exist"}` | **Free.** |

### 2.2 Corrections to prior evidence

| Claim in prior analysis | Verified today | Correction |
|-------------------------|----------------|------------|
| internet-monitor "active (push 2026-09-17, CI, 63★)", "momentum" | Releases: v0.10.1 (2026-06-01) is the latest of 3. Commits since: dependabot bumps (Jul–Aug) + "Add custom sponsorship URL to FUNDING.yml" (2026-09-17). 0 open issues, 1 fork. | **Stalled since 2026-06-01.** `pushed_at` was a sponsorship commit, not development. Threat level downgraded from "active incumbent" to "stalled incumbent." |
| internet-monitor "MIT (README; repo metadata `license: null` — verify before relying on it)" | Repo root file list contains **no LICENSE file**. README line 6 badge `License: MIT` links to `LICENSE` (404); README line 453 asserts "MIT — Free for personal and commercial use." | **MIT is an unsupported claim**; legally the repo is all-rights-reserved until a license file is added. (Consequence: it cannot be forked as-is; its features remain freely imitable — competitive threat unchanged.) |
| PingPlotter "commercial, years old, mature" | Fetched [pingplotter.com](https://www.pingplotter.com/) today: © 1998–2026 Pingman Tools, LLC; desktop (macOS/Windows) + **cloud SaaS**; enterprise customers (Tesco, AT&T, Wells Fargo, Toyota); "prove to IT or your ISP where the issues are." | Confirmed, and sharpened: **no headless/server/CLI product exists** — the commercial incumbent is structurally desktop/SaaS too. The wedge is closed off from both sides. |
| (envdrift) dotenv-linter "narrow (single-file linting)" | [dotenv-linter/dotenv-linter](https://github.com/dotenv-linter/dotenv-linter): Rust, **2,113★**, created 2019, last push **2026-02-05** (8 months), 41 open issues, not archived. | Established but **in maintenance mode**; 41 open issues = live unmet demand. The cross-project drift wedge (envdrift) is intact. |

### 2.3 New evidence: `netband` — the decisive datum

[netband](https://crates.io/crates/netband) — "CLI ping, packet loss, and bandwidth tests with continuous network monitoring." Rust, Apache-2.0, created 2026-08-31, updated 2026-09-18, 120 downloads, CI + Codecov, checksum-verified installer.

| Dimension | netband (shipped) | netproof v1 (planned) |
|-----------|-------------------|----------------------|
| Run mode | **systemd service** (non-root dynamic user, `StateDirectory=/var/lib/netband`) + interactive CLI | headless daemon + CLI |
| Persistence | **CSV journals** (`/var/lib/netband/journals/run/`), scheduler accounting | SQLite, 90-day retention |
| Probes | **ICMP-first** ("Ping needs permission to open ICMP sockets" — a whole setup section), NDT7 bandwidth (M-Lab) | **TCP/HTTP/DNS default, zero permissions**, ICMP optional |
| Detection | "tests triggered by degraded connectivity" (event-triggered tests) | outage windows + **per-layer attribution** + flap detection |
| Output | terminal + **CSV only** | **self-contained HTML report + CSV** (the attachable artifact) |
| Platforms | Linux x86_64/aarch64 only | Linux + macOS |

**Reading:**
1. **Validation:** an independent, competent Rust builder shipped the headless-server positioning six weeks ago with no competition — the wedge context is real demand, not a theory.
2. **Narrowing:** the differentiator vs netband is no longer "headless" (it has that). It is the **evidence layer**: zero-permission probes (netband's ICMP-first design carries exactly the permission friction the prior analysis flagged as risk D.2.1), per-layer attribution, outage windows, and the report artifact. A raw ping CSV still requires the user to interpret it and build the ISP argument; the case file is the adoption hook.
3. **Urgency:** attribution + report is the obvious next step for netband. The window is open but watched — the T-0 pre-publish gate (§7) is now load-bearing.

---

## 3. Meta-Audit — Blind Spots in the Prior Analysis Document

| # | Blind spot | Why it matters | Correction |
|---|-----------|----------------|------------|
| M1 | Gap re-verification was **search-driven** (one web search found internet-monitor), not a systematic, reproducible scan | Missed `netband` — the closest Rust entrant, created 6 weeks before this audit | Gap check becomes a **scripted query set** (Appendix A), re-run at T-0 and T+90 |
| M2 | "Active/momentum" inferred from `pushed_at` | The latest push was a `FUNDING.yml` commit; feature development stopped 4 months earlier | Always read commit history + release dates, not the push timestamp |
| M3 | License left as "verify before relying on it" — never verified | The MIT claim is unsupported (no LICENSE file) | Verified §2.2; no decision impact, evidence corrected |
| M4 | Positioning copy leads with "**Rust** single binary" | Language is not the wedge: netband is Rust and still doesn't own the evidence half; Go binaries are equally static. "Rust" is a distribution/marketing attribute | Lead with: **no server, no port, no tray, no dashboard, zero-permission probes — one static binary.** "Rust" is a footnote |
| M5 | Docker appears in positioning but never in the design | (a) A DNS probe inside a container measures the **container's resolver** (Docker built-in or host-forwarded), not necessarily what user apps see — an evidence-integrity issue; (b) a container on the LAN is the *ideal* vantage point for ISP evidence (isolated from host-OS flakiness) | D2: record the resolver per DNS probe and in the report. D3: ship a Dockerfile (trivial for a static binary) + systemd unit |
| M6 | Kill criterion "audience turns out to be desktop-consumer" is not measurable | An unmeasurable kill criterion is decoration | Replaced with the T+30 rule (§7) |
| M7 | 5-day / ~2k-LOC effort stated without risk breakdown | Where the time actually goes: HTML report renderer (inline-SVG sparklines, no chart lib) and daemon lifecycle (pidfile lock, signals, log rotation) are the overrun risks | Keep 5 days; renderer flagged as the slip risk |

---

## 4. The Wedge, Precisely

**One sentence:** the tool that runs where the incumbents *cannot run* (headless servers, Docker, CI, SSH) and produces what they *cannot produce* (a neutral, attachable, per-layer evidence report with zero-permission probes).

| Competitor | Why it cannot take the wedge |
|------------|------------------------------|
| internet-monitor (Go, stalled) | Architecture is the opposite of the wedge: tray + `localhost:8765` dashboard + WebKitGTK; Linux build is tray-only; no headless item on roadmap; no feature commits since 2026-06-01 |
| PingPlotter (commercial, 1998–) | Desktop (macOS/Windows) + cloud SaaS + paid; business/enterprise focus; no server/CLI/headless product line |
| netband (Rust, 6 weeks old) | Has headless + persistence; lacks zero-permission default probes (ICMP-first), per-layer attribution, outage windows, HTML report, macOS. The obvious next roadmap step — which is why we ship now |

**Positioning line:** *"netband tells you the network is bad right now. netproof proves it was bad last Tuesday."*

---

## 5. Final v1 Specification (delta from prior analysis)

Retained unchanged from the prior analysis: probe layer (DNS/TCP/HTTP, ICMP optional off-by-default), detection layer (windows, attribution, flap, WiFi-vs-upstream), storage (SQLite, 90-day cap, clock integrity), report layer (self-contained HTML + CSV, multi-target defaults, optional SHA-256 manifest), CLI (`start | stop | status | report | selftest`), no local server, no root by default, Linux + macOS, ~2k LOC, 5-day plan.

**Deltas:**
- **D1 — `--once` one-shot probe mode** (one probe round, print, exit): CI parity with netband, and it doubles as the `--selftest` primitive.
- **D2 — Resolver provenance:** record which resolver each DNS probe used; state it in the report header. Required for Docker correctness (M5) and for pre-empting "your test used a different resolver" dismissal.
- **D3 — Ship, don't build, deployment:** repo includes a `systemd` unit and a `Dockerfile` (static binary → trivial image). No built-in auto-start at v1.
- **D4 — Positioning copy:** lead with no-server/no-port/no-tray + zero-permission probes; "Rust" is a footnote (M4).
- **D5 — Non-goals unchanged:** no GUI, no speed tests, no webhooks, no Windows, no auto-update. (Netband has speed tests; they remain a v2 candidate, re-evaluated against §7 gates.)
- **Effort:** 5 days as planned; report renderer flagged as the slip risk (M7).

---

## 6. Final Scoring (corrected matrix)

Adds the **competitive occupancy** axis (1–5, 5 = space most open) identified as missing in the prior analysis §A.4 but never applied.

| Candidate | Correctness | Completeness | Usefulness | Safety | Clarity | Efficiency | Occupancy | Total |
|-----------|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| **A. netproof (evidence wedge)** | 5 | 4 | 5 | 5 | 5 | 4 | 4 | **32/35** |
| C. envdrift | 4 | 4 | 4 | 5 | 5 | 5 | 3 | 30/35 |
| B. diskdoctor | 5 | 4 | 5 | 3 | 4 | 3 | 4 | 28/35 |

- **netproof occupancy = 4, not 5:** the general category has three incumbents; the evidence wedge is open but a live adjacent Rust entrant (netband, 6 weeks old) is one roadmap step away.
- **envdrift occupancy = 3:** dotenv-linter has 2,113★ (strong presence, live demand in 41 open issues) but is in maintenance mode and single-file-only; the cross-project wedge is open; the `.env`-deprecation trend is a headwind.
- **Final ranking: netproof > envdrift > diskdoctor.** Fallback chain on gate failure: envdrift, then diskdoctor.

---

## 7. Pre-Committed Decision Rules

| Gate | When | Test | Action on failure |
|------|------|------|-------------------|
| **T0** | today | — | Start build (5-day plan, §5) |
| **T-0 (pre-publish)** | day 5, before `cargo publish` | Re-run the scripted gap scan (Appendix A) + check netband's repo for attribution/report features | Evidence wedge occupied → **stop, pivot to envdrift** |
| **T+30 (adoption)** | 30 days after publish | crates.io downloads < 200 **and** GitHub stars < 10 **and** zero organic (non-self) mentions → weak signal; feedback uniformly asks "where's the GUI/dashboard?" → wrong wedge | Weak signal → pause, reposition or extend; wrong wedge → **pivot to envdrift** |
| **T+90 (wedge closure)** | 90 days | netband (or any entrant) ships per-layer attribution + attachable report; or internet-monitor revives with a headless mode | **Stop competing head-on**; reassess residual differentiation (zero-permission probes, macOS, Rust distribution) or fold into v2 of the incumbent's absence |

---

## 8. Why GO Is the Right Call

- **Cost ≈ 5 days** of an experienced Rust developer; ~2k LOC; no server, no CGO, no data risk, no safety surface (E5's red-team constraints already baked into the spec).
- **Downside is bounded:** worst case is a small open-source tool with modest adoption. There is no irreversible commitment.
- **The wedge is structural, not aspirational:** both cross-language incumbents are architecturally incapable of the target context (tray/localhost-server; desktop/SaaS), and the closest Rust entrant — six weeks old, well-engineered — validates the context while leaving the entire evidence layer unoccupied.
- **Demand signals are live:** netband's existence (independent builder, 6 weeks, no competition), dotenv-linter's 2,113★ on a narrower pain, and the original r/sysadmin pain thread.
- **The name is free** and on-message (`netproof`).
- **Asymmetry:** cheap to try, pre-committed exits, and the window has a clock (netband's roadmap). The only way GO loses badly is by not shipping — which a conditional verdict risks.

---

## 9. Master Map

```
FINAL VERDICT — netproof
├── 1 ADJUDICATION ..................... conditional GO → unconditional GO (condition was a spec, not a risk)
├── 2 LIVE RE-VERIFICATION (2026-10-15)
│   ├── 2.1 Kill criteria .............. KC1/KC3 not triggered; name free; KC2 replaced by T+30 rule
│   ├── 2.2 Corrections ................ internet-monitor stalled + MIT unsupported; PingPlotter desktop/SaaS-only; dotenv-linter maintenance mode
│   └── 2.3 netband (NEW) .............. headless half taken; wedge narrows to EVIDENCE layer; urgency up
├── 3 META-AUDIT (M1–M7) ............... search-driven gap check, pushed_at fallacy, Rust-lead copy, Docker resolver, unmeasurable KC, effort risk
├── 4 WEDGE ............................ headless context + zero-permission probes + per-layer attribution + attachable report
├── 5 SPEC v1 (D1–D5) .................. --once, resolver provenance, unit+Dockerfile, copy, non-goals; 5 days
├── 6 SCORING (7-axis) ................. netproof 32 > envdrift 30 > diskdoctor 28
├── 7 GATES ............................ T0 build / T-0 gap scan / T+30 adoption / T+90 wedge closure
└── 8 DECISION THEORY .................. bounded cost, structural wedge, live demand, window has a clock → GO
```

---

## 10. Appendix A — Scripted Gap Scan (reproducible; re-run at T-0 and T+90)

```bash
UA="Mozilla/5.0 (gap-scan)"
# 1. Name availability — expect 404 + "crate `netproof` does not exist"
curl -s -H "User-Agent: $UA" -w "\nHTTP %{http_code}\n" https://crates.io/api/v1/crates/netproof
# 2. Rust-space scan — expect NO result that is (a) single-binary, (b) persistent outage
#    logging, (c) per-layer attribution or attachable report. Near-misses with exclusion reasons:
for q in "ping+logger" "outage" "network+monitor" "probe+logger" "latency+monitor" "internet+monitor" "ping"; do
  curl -s -H "User-Agent: $UA" "https://crates.io/api/v1/crates?q=$q&per_page=15" \
   | python3 -c "import json,sys; [print(c['name'],'|',str(c.get('updated_at'))[:10],'|',(c.get('description') or '')[:90]) for c in json.load(sys.stdin).get('crates',[])]"
done
# 3. Incumbent state — expect: no headless/CLI mode; releases/commits show activity level
curl -s -H "User-Agent: $UA" https://api.github.com/repos/FutureSolutionDev/internet-monitor/releases?per_page=3
curl -s -H "User-Agent: $UA" "https://api.github.com/repos/FutureSolutionDev/internet-monitor/commits?per_page=5"
curl -s -H "User-Agent: $UA" https://raw.githubusercontent.com/FutureSolutionDev/internet-monitor/master/README.md | grep -inE "cli|headless|docker|systemd"
# 4. Closest Rust entrant — expect: no per-layer attribution / outage windows / HTML report
curl -s -H "User-Agent: $UA" https://raw.githubusercontent.com/gregl83/netband/main/README.md
```

**Known near-misses and exclusion reasons (2026-10-15):**

| Crate | Why it does not occupy the wedge |
|-------|----------------------------------|
| netband (Rust) | Closest: headless + persistence. Lacks zero-permission default probes (ICMP-first), attribution, outage windows, HTML report, macOS |
| rustnet-monitor (Rust) | Per-process traffic monitor (TUI + headless JSON) — different category (process-level, not link/ISP-level) |
| sniffnet (Rust, 48k dl) | GUI traffic monitor — different category |
| trippy (Rust) | Live traceroute/ping — live tool, no persistence/report |
| octomon, ekg (Rust) | Live terminal monitors — no persistence, no report |
| epingm (Rust) | Stale (last update 2024-03) |
| ieql (Rust) | Stale (last update 2020); content-monitoring standard, not an outage logger |
| surge-ping, tiny-ping, fastping-rs, ping (Rust) | ICMP libraries/tools — components, not evidence loggers |
| internet-monitor (Go) | Tray + localhost dashboard; no headless mode; stalled since 2026-06-01 |
| PingPlotter (commercial) | Desktop + SaaS; no headless/server product |
| CryPing (Python) | Small TCP-ping dropout tool; no report artifact (already in original research) |

---

## 11. Appendix B — Evidence Log (2026-10-15)

1. crates.io `GET /api/v1/crates/netproof` → `404 {"errors":[{"detail":"crate `netproof` does not exist"}]}` — name free.
2. crates.io search API, 7 query families (`ping+logger`, `outage`, `network+monitor`, `probe+logger`, `latency+monitor`, `internet+monitor`, `ping`) — no Rust outage-evidence-logger; near-miss table in Appendix A.
3. GitHub API `repos/FutureSolutionDev/internet-monitor` — Go, created 2026-05-11, pushed 2026-09-17, 63★, 1 fork, 0 open issues, `license: null`, not archived.
4. GitHub API repo file list — **no LICENSE file** in root (`.air.toml, .github, .gitignore, .golangci.yml, CHANGELOG.md, Makefile, README.ar.md, README.md, …, main.go, …`).
5. GitHub API releases — latest v0.10.1 (2026-06-01); prior v0.10.0, v0.9.4 (same day).
6. GitHub API commits (top 5) — 2026-09-17 "Add custom sponsorship URL to FUNDING.yml"; 2026-08-16 dependabot merge; 2026-07-09 x/crypto bump; 2026-07-03 x/net bump. No feature commits since 2026-06-01.
7. internet-monitor README (raw, 455 lines) — "Two modes: System Tray (background) + Standalone native window"; dashboard `localhost:8765`; Linux binary "Ubuntu/Debian — System Tray"; roadmap: "Windows ARM64 build support", "Mobile-responsive dashboard improvements"; badge `License: MIT` → `LICENSE` (missing); line 453 "MIT — Free for personal and commercial use."
8. [pingplotter.com](https://www.pingplotter.com/) (fetched) — © 1998–2026 Pingman Tools, LLC; Desktop (macOS/Windows) + Cloud (SaaS); enterprise customers; "prove to IT or your ISP where the issues are"; no headless/server/CLI product.
9. GitHub API `repos/dotenv-linter/dotenv-linter` — Rust, 2,113★, created 2019-11-30, pushed 2026-02-05, 41 open issues, not archived.
10. crates.io `GET /api/v1/crates/netband` — repo gregl83/netband, created 2026-08-31, updated 2026-09-18, 120 downloads.
11. netband README (raw) — "Ping, packet loss, and download/upload tests from your terminal"; "continuous pings, scheduled bandwidth tests, and tests triggered by degraded connectivity"; "CSV record you can compare later"; "Built for Linux"; "Ping needs permission to open ICMP sockets"; Apache-2.0.
12. netband `docs/service.md` (raw) — "The example service runs Netband in the foreground under systemd as a non-root dynamic user… saves CSVs in `/var/lib/netband/journals/run/`".
