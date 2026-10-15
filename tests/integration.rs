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
    assert!(out.starts_with("{\"ts\":"), "not JSON: {out}");
    assert!(out.contains("\"all_ok\":true"));
    assert!(out.contains("\"target\":\"tcp-local\""));
    assert!(out.contains("\"ok\":true"));

    tcp_task.abort();
    http_task.abort();
    let _ = std::fs::remove_dir_all(&dir);
}
