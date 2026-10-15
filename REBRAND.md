# Rebrand: netproof → urania

Date: 2026-10-15. The 5 planning docs (netproof-build-roadmap.md,
netproof-deep-diagnosis.md, rust-small-project-*.md) planned the project as
`netproof`. During the build, the owner requested a rebrand to a Norse or
Greek god name with no conflicts. This file records the name scan and the
decision.

## Name scan (crates.io API, 2026-10-15)

| Name | Origin | Fit | crates.io | Conflict found | Verdict |
|---|---|---|---|---|---|
| soteria | Greek (soteria = salvation) | — | **taken** (soteria-tools, symbolic execution toolkit, active) | active project | rejected |
| urdr | Norse Norn of the past | strong | free | [urdr.ai](https://urdr.ai/) live commercial product (Urðr desktop app) | rejected |
| ullr | Norse god of archery | good | free | one niche JS web-components lib | backup |
| fulla | Norse confidante of Frigg | medium | free | fulla.dev live product | rejected |
| vali | Norse avenger | medium | free | [credativ/vali](https://github.com/credativ/vali) — Loki fork (log aggregation, same domain) | rejected |
| forseti / heimdall / muninn / tyr / vidar / mimir / saga / bifrost / gjallarhorn | Norse | — | taken | various | rejected |
| clio | Greek Muse of history | **strong** | — | [clio.com](https://www.clio.com/) major legal-tech SaaS brand | rejected |
| dike | Greek goddess of justice | good | free | DikeDataset malware-analysis platform + English-word collision | rejected |
| themis / hestia / nereus / aletheia / mnemosyne / hecate / metis / calliope / lachesis / clotho / eirene / thea / rhea / horkos / pistis | Greek | — | taken | various | rejected |
| astraea | Greek goddess of justice | good | free | Astraea (Kafka stream-processing library) | rejected |
| euterpe / terpsichore / melpomene / atropos | Greek Muses/Fates | weak | free | minor | rejected (fit) |
| **urania** | **Greek Muse of astronomy** | **strong** | **free (HTTP 404)** | only small Clojure/Scala libs (other languages/domains) | **chosen** |

## Why urania

Urania is the Muse of astronomy — the one who watches the heavens. urania is
a 24/7 watcher of the network's sky that records what it sees, so you can
prove it later:

> netband tells you the network is bad right now.
> urania proves it was bad last Tuesday.

## Renames applied

| netproof | urania |
|---|---|
| crate `netproof` | crate `urania` |
| binary `netproof` | binary `urania` |
| `~/.config/netproof/config.toml` | `~/.config/urania/config.toml` |
| `~/.local/share/netproof/` / `~/Library/Application Support/netproof/` | `~/.local/share/urania/` / `~/Library/Application Support/urania/` |
| `netproof.db` / `netproof.lock` / `netproof.log` | `urania.db` / `urania.lock` / `urania-YYYY-MM-DD` |
| `netproof.service` | `urania.service` |
| Docker entrypoint `/netproof` | `/urania` |
| report header "netproof vX" | "urania vX" |
| tagline "netproof proves it was bad last Tuesday" | "urania proves it was bad last Tuesday" |

## Implementation deviations from the roadmap

- **Schema:** added `flap INTEGER NOT NULL DEFAULT 0` to `outages` (the
  report's Flap column and CloseWindow require it; the roadmap schema omitted
  it).
- **Log rotation:** tracing-appender 0.2 has no size-based rotation; daily
  files with keep-2 are used (info-level volume stays far below the 5 MB
  target).
- **Daemon detach:** the roadmap's macOS double-fork + setpgrp was replaced
  by `setsid()` in `pre_exec` on both platforms (async-signal-safe; achieves
  the same session detachment without forking inside pre_exec).

## Known residual conflicts (accepted)

- `funcool/urania` (Clojure remote data access) and `urania-dev/snapp`
  (Scala) — different languages and domains; no network tooling.
- No trademark search was performed; a formal search is required before
  commercial distribution (see PUBLISH-GATE.md).
