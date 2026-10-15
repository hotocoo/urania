//! End-to-end: real binary, local dummy targets, 1s interval, SIGTERM, report.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn daemon_roundtrip_local_dummies() {
    // Local TCP server (accept + hold).
    let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tcp_addr = tcp_listener.local_addr().unwrap();
    let tcp_task = tokio::spawn(async move {
        loop {
            let (mut s, _) = match tcp_listener.accept().await {
                Ok(x) => x,
                Err(_) => break,
            };
            tokio::spawn(async move {
                let _ = s.read(&mut [0u8; 1]).await;
            });
        }
    });
    // Local HTTP 204 server.
    let http_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_addr = http_listener.local_addr().unwrap();
    let http_task = tokio::spawn(async move {
        loop {
            let (mut s, _) = match http_listener.accept().await {
                Ok(x) => x,
                Err(_) => break,
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf).await;
                let resp =
                    "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = s.write_all(resp.as_bytes()).await;
            });
        }
    });

    let dir = std::env::temp_dir().join(format!("urania-int-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("test.db");
    let log = dir.join("test.log");
    let cfg_path = dir.join("config.toml");
    std::fs::write(
        &cfg_path,
        format!(
            "fail_threshold = 2\nretention_days = 1\ndb_path = \"{db}\"\nlog_path = \"{log}\"\n\n[[target]]\nname = \"tcp-local\"\ntype = \"tcp\"\nendpoint = \"{tcp}\"\ninterval = 1\ntimeout = 2\n\n[[target]]\nname = \"http-local\"\ntype = \"http\"\nendpoint = \"http://{http}\"\ninterval = 1\ntimeout = 2\n",
            db = db.display(),
            log = log.display(),
            tcp = tcp_addr,
            http = http_addr,
        ),
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_urania");
    let mut child = std::process::Command::new(bin)
        .args(["run", "--foreground", "--config"])
        .arg(&cfg_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    // Wait for the pidfile (lock acquired).
    let lock_path = dir.join("urania.lock");
    let mut pid = None;
    for _ in 0..100 {
        if let Ok(s) = std::fs::read_to_string(&lock_path)
            && let Ok(p) = s.trim().parse::<u32>()
        {
            pid = Some(p);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let pid = match pid {
        Some(p) => p,
        None => {
            let _ = child.kill();
            panic!("daemon did not start within 10s");
        }
    };

    // Let it probe for a few rounds (1s interval).
    std::thread::sleep(Duration::from_millis(3_500));

    // status --check: exit 0 while running.
    let st = std::process::Command::new(bin)
        .args(["status", "--check", "--config"])
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "status --check should exit 0 while running: {}",
        String::from_utf8_lossy(&st.stdout)
    );
    assert!(String::from_utf8_lossy(&st.stdout).contains("running"));

    // status --json: valid JSON, running, targets present. Poll until the
    // 5s batch flush has landed probe rows in the db (timing-independent).
    let mut v = serde_json::Value::Null;
    for _ in 0..20 {
        let st_json = std::process::Command::new(bin)
            .args(["status", "--json", "--config"])
            .arg(&cfg_path)
            .output()
            .unwrap();
        assert!(
            st_json.status.success(),
            "status --json should exit 0 while running: {}",
            String::from_utf8_lossy(&st_json.stderr)
        );
        v = serde_json::from_slice(&st_json.stdout).unwrap_or_else(|e| {
            panic!(
                "status --json not valid JSON: {e}: {}",
                String::from_utf8_lossy(&st_json.stdout)
            )
        });
        if v["db"]["rows"].as_u64().unwrap_or(0) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    assert_eq!(v["running"], serde_json::json!(true));
    assert!(v["pid"].is_number(), "pid: {v}");
    assert!(
        v["db"]["rows"].as_u64().unwrap_or(0) > 0,
        "no probe rows after 10s: {v}"
    );
    let targets = v["targets"].as_array().unwrap();
    assert!(
        targets
            .iter()
            .any(|t| t["name"] == "tcp-local" && t["last"]["ok"].is_boolean()),
        "tcp-local target with last probe: {v}"
    );

    // SIGTERM → graceful stop.
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    let status = child.wait().unwrap();
    assert!(status.success(), "daemon exited with {status}");

    // status --check: exit 1 after the stop.
    let st = std::process::Command::new(bin)
        .args(["status", "--check", "--config"])
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(
        st.status.code() == Some(1),
        "status --check should exit 1 when not running, got {:?}",
        st.status.code()
    );

    // Report over the collected data, with a SHA-256 manifest.
    let out_html = dir.join("report.html");
    let out_csv = dir.join("report.csv");
    let rep = std::process::Command::new(bin)
        .args(["report", "--since", "24h", "--to"])
        .arg(&out_html)
        .arg("--csv")
        .arg(&out_csv)
        .arg("--manifest")
        .arg("--config")
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(
        rep.status.success(),
        "report failed: {}\n{}",
        String::from_utf8_lossy(&rep.stdout),
        String::from_utf8_lossy(&rep.stderr)
    );
    let html = std::fs::read_to_string(&out_html).unwrap();
    assert!(html.contains("tcp-local"));
    assert!(html.contains("http-local"));
    let csv_text = std::fs::read_to_string(&out_csv).unwrap();
    assert!(csv_text.lines().count() > 3, "csv too small: {csv_text}");
    let manifest = dir.join("report.html.sha256");
    assert!(manifest.exists(), "manifest not written");

    // Evidence integrity: verify passes on the fresh report...
    let ver = std::process::Command::new(bin)
        .args(["verify"])
        .arg(&out_html)
        .arg("--config")
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(
        ver.status.success(),
        "verify failed: {}\n{}",
        String::from_utf8_lossy(&ver.stdout),
        String::from_utf8_lossy(&ver.stderr)
    );

    // Absolute incident window: --from (far past) with --end defaulting to
    // now must cover everything the daemon wrote, be labeled as an
    // absolute window, and pass verify.
    let win_html = dir.join("window.html");
    let win = std::process::Command::new(bin)
        .args(["report", "--from", "2020-01-01T00:00:00Z", "--to"])
        .arg(&win_html)
        .arg("--manifest")
        .arg("--config")
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(
        win.status.success(),
        "window report failed: {}\n{}",
        String::from_utf8_lossy(&win.stdout),
        String::from_utf8_lossy(&win.stderr)
    );
    let win_text = std::fs::read_to_string(&win_html).unwrap();
    assert!(win_text.contains("absolute window"));
    assert!(win_text.contains("tcp-local"));
    let win_ver = std::process::Command::new(bin)
        .args(["verify", "--quiet"])
        .arg(&win_html)
        .arg("--config")
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(
        win_ver.status.success(),
        "window verify failed: {}",
        String::from_utf8_lossy(&win_ver.stderr)
    );

    // ...and fails (exit 1) when the report is tampered with.
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&out_html)
            .unwrap();
        f.write_all(b"<!-- tampered -->").unwrap();
    }
    let ver = std::process::Command::new(bin)
        .args(["verify"])
        .arg(&out_html)
        .arg("--config")
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(
        ver.status.code() == Some(1),
        "tampered verify should exit 1, got {:?}: {}",
        ver.status.code(),
        String::from_utf8_lossy(&ver.stdout)
    );

    // once: one probe round against the still-running local TCP dummy.
    let once = std::process::Command::new(bin)
        .args(["once", "--target", "tcp-local", "--config"])
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(
        once.status.success(),
        "once failed: {}\n{}",
        String::from_utf8_lossy(&once.stdout),
        String::from_utf8_lossy(&once.stderr)
    );
    assert!(String::from_utf8_lossy(&once.stdout).contains("tcp-local"));

    // once --json: machine-readable output, structurally valid.
    let once_json = std::process::Command::new(bin)
        .args(["once", "--json", "--target", "tcp-local", "--config"])
        .arg(&cfg_path)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&once_json.stdout);
    assert!(once_json.status.success(), "once --json failed: {out}");
    let oj: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("once --json not valid JSON: {e}: {out}"));
    assert_eq!(oj["all_ok"], serde_json::json!(true));
    assert!(
        oj["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["target"] == "tcp-local" && r["ok"] == serde_json::json!(true)),
        "tcp-local ok result: {out}"
    );

    // config --json: the effective config, machine-readable.
    let cfg_json = std::process::Command::new(bin)
        .args(["config", "--json", "--config"])
        .arg(&cfg_path)
        .output()
        .unwrap();
    let cout = String::from_utf8_lossy(&cfg_json.stdout);
    assert!(cfg_json.status.success(), "config --json failed: {cout}");
    let cj: serde_json::Value = serde_json::from_str(&cout)
        .unwrap_or_else(|e| panic!("config --json not valid JSON: {e}: {cout}"));
    assert_eq!(cj["config"], cfg_path.to_string_lossy().to_string());
    let ctargets = cj["targets"].as_array().unwrap();
    assert!(
        ctargets
            .iter()
            .any(|t| t["name"] == "tcp-local" && t["interval_s"].is_number()),
        "tcp-local with interval: {cout}"
    );

    // once --quiet: silent on success (exit code only).
    let once_q = std::process::Command::new(bin)
        .args(["once", "--quiet", "--target", "tcp-local", "--config"])
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(once_q.status.success(), "once --quiet should exit 0");
    assert!(
        once_q.stdout.is_empty(),
        "once --quiet should be silent: {}",
        String::from_utf8_lossy(&once_q.stdout)
    );

    // selftest --offline: full evidence chain without network.
    let st_off = std::process::Command::new(bin)
        .args(["selftest", "--offline"])
        .output()
        .unwrap();
    assert!(
        st_off.status.success(),
        "selftest --offline failed: {}",
        String::from_utf8_lossy(&st_off.stdout)
    );
    assert!(String::from_utf8_lossy(&st_off.stdout).contains("selftest: PASS"));

    // report --stdout: HTML on stdout, status line on stderr.
    let rep_s = std::process::Command::new(bin)
        .args(["report", "--stdout", "--since", "24h", "--config"])
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(rep_s.status.success(), "report --stdout failed");
    let html_out = String::from_utf8_lossy(&rep_s.stdout);
    assert!(
        html_out.starts_with("<!doctype html>"),
        "stdout must be pure HTML"
    );
    assert!(html_out.ends_with("</html>\n") || html_out.ends_with("</html>"));
    assert!(
        !html_out.contains("report:"),
        "status line must not pollute stdout"
    );
    assert!(
        String::from_utf8_lossy(&rep_s.stderr).contains("report: -"),
        "status line goes to stderr"
    );

    // report --since + --from: mutually exclusive, explicit error.
    let conflict = std::process::Command::new(bin)
        .args([
            "report",
            "--since",
            "1h",
            "--from",
            "2020-01-01T00:00:00Z",
            "--to",
            "/tmp/should-not-exist.html",
        ])
        .arg("--config")
        .arg(&cfg_path)
        .output()
        .unwrap();
    assert!(
        conflict.status.code() == Some(1),
        "since+from should exit 1, got {:?}",
        conflict.status.code()
    );
    assert!(
        String::from_utf8_lossy(&conflict.stderr).contains("mutually exclusive"),
        "stderr: {}",
        String::from_utf8_lossy(&conflict.stderr)
    );

    tcp_task.abort();
    http_task.abort();
    let _ = std::fs::remove_dir_all(&dir);
}
