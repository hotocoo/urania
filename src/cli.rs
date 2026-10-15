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
    Status,
    /// Render the evidence report (self-contained HTML, optional CSV + SHA-256 manifest)
    Report {
        /// Period: 24h | 7d | 30d | 90d
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
    /// Run one probe round, print a table, and exit (0 = all ok, 1 = any failed)
    Once {
        /// Only this target name
        #[arg(long)]
        target: Option<String>,
        /// Only this probe type: dns | tcp | http | icmp
        #[arg(long = "type")]
        probe_type: Option<String>,
    },
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
