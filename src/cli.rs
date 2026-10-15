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
    },
    /// Render the evidence report (self-contained HTML, optional CSV + SHA-256 manifest)
    Report {
        /// Period: 1h | 24h | 7d | 30d | 90d
        #[arg(long, default_value = "24h")]
        since: String,
        /// Output HTML path (default: ./urania-report.html)
        #[arg(long)]
        to: Option<PathBuf>,
        /// Also write CSV to this path
        #[arg(long)]
        csv: Option<PathBuf>,
        /// Write a SHA-256 manifest alongside the outputs
        #[arg(long)]
        manifest: bool,
    },
    /// Verify a report against its SHA-256 manifest (evidence integrity)
    Verify {
        /// Report HTML path (manifest expected at <path>.sha256)
        report: PathBuf,
        /// Explicit manifest path (default: <report>.sha256)
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    /// Run one probe round, print a table, and exit (0 = all ok, 1 = any failed)
    Once {
        /// Only this target name
        #[arg(long)]
        target: Option<String>,
        /// Only this probe type: dns | tcp | http | icmp
        #[arg(long = "type")]
        probe_type: Option<String>,
    },
    /// Show the effective configuration (resolved file + built-in defaults)
    Config,
    /// Round-trip self-test against known-good targets
    Selftest,
    /// Hidden: the real daemon loop (systemd / Docker)
    #[command(hide = true)]
    Run {
        /// Required; the daemon loop only runs in the foreground
        #[arg(long)]
        foreground: bool,
    },
}
