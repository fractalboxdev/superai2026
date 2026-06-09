//! Supervisor (spec 10 §2): spawns the bundled `sync` subcommand for the
//! chosen role, tails its output, health-checks it, and restarts on crash
//! with backoff. Surfaces `starting · healthy · degraded · stopped`.

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch;

use crate::settings::{AppSettings, Role};
use crate::sidecar;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Starting,
    Healthy,
    Degraded,
    Stopped,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub status: Status,
    pub detail: String,
    pub pid: Option<u32>,
    pub restarts: u32,
}

const LOG_RING_MAX: usize = 1000;
const LOG_FILE_MAX_BYTES: u64 = 5 * 1024 * 1024;

pub struct Supervisor {
    inner: Arc<Inner>,
}

struct Inner {
    app: AppHandle,
    snap: Mutex<Snapshot>,
    logs: Mutex<VecDeque<String>>,
    desired: watch::Sender<bool>,
    loop_alive: Mutex<bool>,
    log_path: PathBuf,
}

impl Supervisor {
    pub fn new(app: AppHandle) -> Self {
        let (tx, _rx) = watch::channel(false);
        let log_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Logs/Contextful");
        Self {
            inner: Arc::new(Inner {
                app,
                snap: Mutex::new(Snapshot {
                    status: Status::Stopped,
                    detail: "Not running.".into(),
                    pid: None,
                    restarts: 0,
                }),
                logs: Mutex::new(VecDeque::new()),
                desired: tx,
                loop_alive: Mutex::new(false),
                log_path: log_dir.join("sync.log"),
            }),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.snap.lock().unwrap().clone()
    }

    pub fn logs(&self, limit: usize) -> Vec<String> {
        let ring = self.inner.logs.lock().unwrap();
        ring.iter().rev().take(limit).rev().cloned().collect()
    }

    pub fn start(&self) {
        // send_replace: a plain send() fails while no receiver is alive
        // (i.e. before the first run_loop subscribes), losing the request.
        self.inner.desired.send_replace(true);
        let mut alive = self.inner.loop_alive.lock().unwrap();
        if !*alive {
            *alive = true;
            let inner = self.inner.clone();
            tauri::async_runtime::spawn(run_loop(inner));
        }
    }

    pub fn stop(&self) {
        self.inner.desired.send_replace(false);
    }

    pub async fn restart(&self) {
        self.stop();
        // give the loop a moment to reap the child, then relaunch
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.start();
    }
}

/// Exponential backoff, capped at 60s: 1, 2, 4, … 60.
pub fn backoff_secs(attempt: u32) -> u64 {
    (1u64 << attempt.min(6)).min(60)
}

/// Pidfile next to the app config. If the app dies without cleanup (SIGKILL,
/// crash), the next launch reaps the orphaned child before spawning a new one
/// — no double-spawn, no stale port bind.
fn pidfile() -> PathBuf {
    AppSettings::path().with_file_name("sync.pid")
}

fn write_pidfile(pid: u32) {
    let path = pidfile();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, pid.to_string());
}

fn clear_pidfile() {
    let _ = std::fs::remove_file(pidfile());
}

fn reap_stale_child(inner: &Inner) {
    let Ok(raw) = std::fs::read_to_string(pidfile()) else {
        return;
    };
    let Ok(pid) = raw.trim().parse::<i32>() else {
        clear_pidfile();
        return;
    };
    // Only kill it if it's really our sidecar, not a recycled pid.
    let is_sync = std::process::Command::new("/bin/ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().ends_with("sync"))
        .unwrap_or(false);
    if is_sync {
        inner.log_line(format!(
            "[supervisor] reaping orphaned sync from a previous run (pid {pid})"
        ));
        unsafe {
            libc_kill(pid);
        }
    }
    clear_pidfile();
}

unsafe fn libc_kill(pid: i32) {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    kill(pid, 15); // SIGTERM
}

/// Sidecar argv for the configured role (spec 10 §1).
pub fn build_args(s: &AppSettings) -> Vec<String> {
    match s.role {
        Role::Host => vec![
            "serve".into(),
            "--addr".into(),
            s.relay_addr.clone(),
            "--with-mcp".into(),
        ],
        Role::Member => vec![
            "client".into(),
            "--addr".into(),
            s.relay_addr.trim_start_matches("ws://").to_string(),
            "--doc".into(),
            s.doc.clone(),
            "--principal".into(),
            s.principal.clone(),
        ],
    }
}

/// Address probed by the health check. The host binds locally; members are
/// healthy when the *relay* is reachable.
pub fn health_addr(s: &AppSettings) -> String {
    match s.role {
        Role::Host => format!("127.0.0.1:{}", crate::tailscale::port_of(&s.relay_addr)),
        Role::Member => s.relay_addr.trim_start_matches("ws://").to_string(),
    }
}

async fn tcp_ok(addr: &str) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(2), tokio::net::TcpStream::connect(addr)).await,
        Ok(Ok(_))
    )
}

impl Inner {
    fn set(&self, status: Status, detail: impl Into<String>, pid: Option<u32>) {
        let snap = {
            let mut s = self.snap.lock().unwrap();
            s.status = status;
            s.detail = detail.into();
            s.pid = pid;
            s.clone()
        };
        let _ = self.app.emit("supervisor:status", snap.clone());
        crate::tray::reflect(&self.app, snap.status);
    }

    fn bump_restarts(&self) {
        self.snap.lock().unwrap().restarts += 1;
    }

    fn log_line(&self, line: String) {
        {
            let mut ring = self.logs.lock().unwrap();
            if ring.len() >= LOG_RING_MAX {
                ring.pop_front();
            }
            ring.push_back(line.clone());
        }
        self.append_log_file(&line);
        let _ = self.app.emit("supervisor:log", line);
    }

    fn append_log_file(&self, line: &str) {
        if let Some(dir) = self.log_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(meta) = std::fs::metadata(&self.log_path) {
            if meta.len() > LOG_FILE_MAX_BYTES {
                let _ = std::fs::rename(&self.log_path, self.log_path.with_extension("log.1"));
            }
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

enum Event {
    Exited(std::io::Result<std::process::ExitStatus>),
    DesiredChanged,
    HealthTick,
}

async fn run_loop(inner: Arc<Inner>) {
    let mut rx = inner.desired.subscribe();
    let mut attempt: u32 = 0;
    let mut first_spawn = true;

    reap_stale_child(&inner);

    'outer: loop {
        if !*rx.borrow() {
            break;
        }
        let settings = AppSettings::load();
        let Some(bin) = sidecar::resolve() else {
            inner.set(
                Status::Stopped,
                "The sync binary is missing from the app bundle.",
                None,
            );
            break;
        };

        if !first_spawn {
            inner.bump_restarts();
        }
        first_spawn = false;

        let args = build_args(&settings);
        inner.log_line(format!(
            "[supervisor] launching {} {}",
            bin.display(),
            args.join(" ")
        ));
        inner.set(Status::Starting, "Launching sync…", None);

        let mut cmd = tokio::process::Command::new(&bin);
        cmd.args(&args)
            .env("RUST_LOG", "info")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        if let Some(home) = settings.brain_home_expanded() {
            cmd.env("CONTEXTFUL_HOME", home);
        }
        if settings.inference != "stub" {
            cmd.env("CONTEXTFUL_INFERENCE", settings.inference.clone());
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let delay = backoff_secs(attempt);
                attempt += 1;
                inner.log_line(format!("[supervisor] spawn failed: {e}"));
                inner.set(
                    Status::Starting,
                    format!("Couldn’t launch sync ({e}); retrying in {delay}s."),
                    None,
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }
        };
        let pid = child.id();
        if let Some(p) = pid {
            write_pidfile(p);
        }
        inner.set(Status::Starting, "Waiting for first health check…", pid);

        if let Some(out) = child.stdout.take() {
            let inner2 = inner.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(out).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    inner2.log_line(line);
                }
            });
        }
        if let Some(err) = child.stderr.take() {
            let inner2 = inner.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    inner2.log_line(line);
                }
            });
        }

        let health_target = health_addr(&settings);
        let mut tick = tokio::time::interval(Duration::from_secs(3));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut stopping = false;

        loop {
            let ev = tokio::select! {
                res = child.wait() => Event::Exited(res),
                changed = rx.changed() => {
                    if changed.is_err() { Event::Exited(child.wait().await) } else { Event::DesiredChanged }
                }
                _ = tick.tick() => Event::HealthTick,
            };
            match ev {
                Event::DesiredChanged => {
                    if !*rx.borrow() {
                        stopping = true;
                        inner.log_line("[supervisor] stop requested".into());
                        let _ = child.start_kill();
                    }
                }
                Event::HealthTick => {
                    if stopping {
                        continue;
                    }
                    if tcp_ok(&health_target).await {
                        attempt = 0;
                        let detail = match settings.role {
                            Role::Host => "Relay and brain are up.",
                            Role::Member => "Connected to the host relay.",
                        };
                        inner.set(Status::Healthy, detail, pid);
                    } else {
                        let detail = match settings.role {
                            Role::Host => "Running, but the relay port isn’t answering yet.",
                            Role::Member => "Running, but the host relay is unreachable.",
                        };
                        inner.set(Status::Degraded, detail, pid);
                    }
                }
                Event::Exited(res) => {
                    clear_pidfile();
                    let code = res
                        .map(|s| s.code().map_or("signal".into(), |c| c.to_string()))
                        .unwrap_or_else(|e| format!("wait error: {e}"));
                    inner.log_line(format!("[supervisor] sync exited ({code})"));
                    if stopping || !*rx.borrow() {
                        inner.set(Status::Stopped, "Stopped.", None);
                        // wait until someone wants us running again
                        while !*rx.borrow() {
                            if rx.changed().await.is_err() {
                                break 'outer;
                            }
                        }
                        attempt = 0;
                        continue 'outer;
                    }
                    let delay = backoff_secs(attempt);
                    attempt += 1;
                    inner.set(
                        Status::Starting,
                        format!("sync exited ({code}); restarting in {delay}s."),
                        None,
                    );
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    continue 'outer;
                }
            }
        }
    }

    *inner.loop_alive.lock().unwrap() = false;
    let snap = inner.snap.lock().unwrap().clone();
    if snap.status != Status::Stopped {
        inner.set(Status::Stopped, "Stopped.", None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(role: Role) -> AppSettings {
        AppSettings {
            role,
            relay_addr: match role {
                Role::Host => "0.0.0.0:7878".into(),
                Role::Member => "studio.tail1234.ts.net:7878".into(),
            },
            ..AppSettings::default()
        }
    }

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!(backoff_secs(0), 1);
        assert_eq!(backoff_secs(1), 2);
        assert_eq!(backoff_secs(4), 16);
        assert_eq!(backoff_secs(6), 60);
        assert_eq!(backoff_secs(20), 60);
    }

    #[test]
    fn host_args_serve_with_mcp() {
        let args = build_args(&settings(Role::Host));
        assert_eq!(args, ["serve", "--addr", "0.0.0.0:7878", "--with-mcp"]);
    }

    #[test]
    fn member_args_client_strips_scheme() {
        let mut s = settings(Role::Member);
        s.relay_addr = "ws://studio.tail1234.ts.net:7878".into();
        s.principal = "agent:cto/1".into();
        let args = build_args(&s);
        assert_eq!(
            args,
            [
                "client",
                "--addr",
                "studio.tail1234.ts.net:7878",
                "--doc",
                "finops",
                "--principal",
                "agent:cto/1"
            ]
        );
    }

    #[test]
    fn health_addr_host_probes_localhost() {
        assert_eq!(health_addr(&settings(Role::Host)), "127.0.0.1:7878");
        assert_eq!(
            health_addr(&settings(Role::Member)),
            "studio.tail1234.ts.net:7878"
        );
    }
}
