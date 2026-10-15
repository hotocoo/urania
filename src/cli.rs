//! CLI surface (roadmap §4.4).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "urania",
    version,
    about = "Headless, zero-permission network outage evidence logger"
)]
pub struct Cli {
    /// Path to config file (default: ~/.config/urania/config.toml)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Start the background daemon
    Start {
        /// Run in the foreground instead of detaching
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the daemon (SIGTERM, graceful, up to 10s)
    Stop,
    /// Show daemon, database, and target status
    Status {
        /// Scripting: print only the state, exit 0 = running, 1 = not running
        #[arg(long)]
        check: bool,
        /// Machine-readable JSON (exit 0 = running, 1 = not running)
        #[arg(long)]
        json: bool,
    },
    /// Render the evidence report (self-contained HTML, optional CSV + SHA-256 manifest)
    Report {
        /// Period: 1h | 24h | 7d | 30d | 90d (default 24h; mutually
        /// exclusive with --from)
        #[arg(long)]
        since: Option<String>,
        /// Absolute window start, RFC3339 (e.g. 2026-10-15T08:00:00+08:00);
        /// renders the incident window instead of a trailing period
        #[arg(long)]
        from: Option<String>,
        /// Absolute window end, RFC3339 (default: now)
        #[arg(long)]
        end: Option<String>,
        /// Output HTML path (default: ./urania-report.html)
        #[arg(long)]
        to: Option<PathBuf>,
        /// Also write CSV to this path
        #[arg(long)]
        csv: Option<PathBuf>,
        /// Write a SHA-256 manifest alongside the outputs
        #[arg(long)]
        manifest: bool,
        /// Print the HTML to stdout instead of a file (CI log
        /// attachments); the evidence chain (--csv/--manifest) needs
        /// files on disk, so clap rejects the combination
        #[arg(long, conflicts_with_all = ["csv", "manifest"])]
        stdout: bool,
        /// Custom report title (e.g. an incident label) shown in the
        /// header; the period is always shown separately
        #[arg(long)]
        title: Option<String>,
    },
    /// Verify a report against its SHA-256 manifest (evidence integrity)
    Verify {
        /// Report HTML path (manifest expected at <path>.sha256)
        report: PathBuf,
        /// Explicit manifest path (default: <report>.sha256)
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Silent on success (exit code only); failures still print
        #[arg(long)]
        quiet: bool,
    },
    /// Run one probe round, print a table, and exit (0 = all ok, 1 = any failed)
    Once {
        /// Only this target name
        #[arg(long)]
        target: Option<String>,
        /// Only this probe type: dns | tcp | http | icmp
        #[arg(long = "type")]
        probe_type: Option<String>,
        /// Machine-readable JSON output instead of the table
        #[arg(long)]
        json: bool,
        /// Silent on success (exit code only); failures still print
        /// (the exit code says what, the output says which)
        #[arg(long)]
        quiet: bool,
    },
    /// Show the effective configuration (resolved file + built-in defaults)
    Config {
        /// Machine-readable JSON for config management/auditing
        #[arg(long)]
        json: bool,
    },
    /// Round-trip self-test against known-good targets
    Selftest {
        /// No network: use a local TCP dummy instead of external targets
        /// (air-gapped installs)
        #[arg(long)]
        offline: bool,
    },
    /// Hidden: the real daemon loop (systemd / Docker)
    #[command(hide = true)]
    Run {
        /// Required; the daemon loop only runs in the foreground
        #[arg(long)]
        foreground: bool,
    },
}
