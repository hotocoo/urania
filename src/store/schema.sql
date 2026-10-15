-- urania schema v1
PRAGMA journal_mode = WAL;
PRAGMA auto_vacuum = INCREMENTAL;

CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS probes (
  id               INTEGER PRIMARY KEY,
  ts               INTEGER NOT NULL,
  ts_mono_delta_ms INTEGER,
  target           TEXT NOT NULL,
  probe_type       TEXT NOT NULL,
  ok               INTEGER NOT NULL,
  latency_ms       REAL,
  detail           TEXT
);
CREATE INDEX IF NOT EXISTS idx_probes_ts ON probes(ts);
CREATE INDEX IF NOT EXISTS idx_probes_target_ts ON probes(target, ts);

-- `flap` is an addition to the roadmap schema (the report's Flap column and
-- CloseWindow need it; the roadmap schema omitted it — see REBRAND.md).
CREATE TABLE IF NOT EXISTS outages (
  id             INTEGER PRIMARY KEY,
  start_ts       INTEGER NOT NULL,
  end_ts         INTEGER,
  target         TEXT NOT NULL,
  layers         TEXT NOT NULL,
  cause          TEXT,
  probes_failed  INTEGER NOT NULL,
  location_class TEXT,
  flap           INTEGER NOT NULL DEFAULT 0
);
