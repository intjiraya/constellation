use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{debug, info, instrument, warn};

const READ_BUF: usize = 8192;
const CHANNEL_CAPACITY: usize = 256;
const DEFAULT_COLS: u16 = 120;
const DEFAULT_ROWS: u16 = 32;
const CLAUDE_BIN: &str = "claude";

const CHILD_REAP_TIMEOUT: Duration = Duration::from_secs(2);

const ENV_ALLOWLIST: &[&str] = &[
    "TERM",
    "COLORTERM",
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "TZ",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_TYPE",
    "USERPROFILE",
    "USERNAME",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMFILES",
    "PROGRAMDATA",
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    "PATHEXT",
];

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClientFrame {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}

pub async fn spawn_resume_bridge(socket: WebSocket, session_id: String, cwd: String, fork: bool) {
    let mut cmd = CommandBuilder::new(CLAUDE_BIN);
    cmd.arg("--resume");
    cmd.arg(&session_id);
    if fork {
        cmd.arg("--fork-session");
    }
    cmd.cwd(&cwd);
    populate_env(&mut cmd);
    bridge(socket, cmd, cwd, Some(session_id)).await;
}

pub async fn spawn_new_chat_bridge(socket: WebSocket, cwd: String) {
    let mut cmd = CommandBuilder::new(CLAUDE_BIN);
    cmd.cwd(&cwd);
    populate_env(&mut cmd);
    bridge(socket, cmd, cwd, None).await;
}

fn populate_env(cmd: &mut CommandBuilder) {
    let mut has_term = false;
    for key in ENV_ALLOWLIST {
        if let Ok(value) = std::env::var(key) {
            cmd.env(*key, value);
            if *key == "TERM" {
                has_term = true;
            }
        }
    }
    if !has_term {
        cmd.env("TERM", "xterm-256color");
    }
}

#[instrument(skip(socket, cmd), fields(session_id = ?session_id_for_log, cwd = %cwd))]
async fn bridge(
    socket: WebSocket,
    cmd: CommandBuilder,
    cwd: String,
    session_id_for_log: Option<String>,
) {
    let started = Instant::now();
    info!("ws-pty bridge opening");
    let (mut ws_tx, mut ws_rx) = socket.split();

    let cwd_path = std::path::Path::new(&cwd);
    if !cwd_path.is_dir() {
        warn!(%cwd, "rejecting bridge: cwd missing");
        let _ = ws_tx
            .send(error_frame(&format!("original cwd is missing: {cwd}")))
            .await;
        let _ = ws_tx.close().await;
        return;
    }
    if !cwd_under_home(cwd_path) {
        warn!(%cwd, "rejecting bridge: cwd not under $HOME");
        let _ = ws_tx
            .send(error_frame(&format!("cwd not under $HOME: {cwd}")))
            .await;
        let _ = ws_tx.close().await;
        return;
    }

    let pty_system = native_pty_system();
    let pair = match pty_system.openpty(PtySize {
        cols: DEFAULT_COLS,
        rows: DEFAULT_ROWS,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "openpty failed");
            let _ = ws_tx
                .send(error_frame(&format!("openpty failed: {e}")))
                .await;
            let _ = ws_tx.close().await;
            return;
        }
    };

    let mut child = match pair.slave.spawn_command(cmd) {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "spawn failed");
            let _ = ws_tx.send(error_frame(&format!("spawn failed: {e}"))).await;
            let _ = ws_tx.close().await;
            return;
        }
    };
    drop(pair.slave);

    let reader = match pair.master.try_clone_reader() {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "reader clone failed");
            let _ = ws_tx
                .send(error_frame(&format!("reader clone failed: {e}")))
                .await;
            let _ = child.kill();
            return;
        }
    };
    let writer = match pair.master.take_writer() {
        Ok(w) => Arc::new(Mutex::new(w)),
        Err(e) => {
            warn!(error = %e, "writer take failed");
            let _ = ws_tx
                .send(error_frame(&format!("writer take failed: {e}")))
                .await;
            let _ = child.kill();
            return;
        }
    };
    let master = Arc::new(Mutex::new(pair.master));

    let (out_tx, mut out_rx) = mpsc::channel::<PtyOutput>(CHANNEL_CAPACITY);
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = vec![0u8; READ_BUF];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    let _ = out_tx.blocking_send(PtyOutput::Eof);
                    break;
                }
                Ok(n) => {
                    if out_tx
                        .blocking_send(PtyOutput::Bytes(buf[..n].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    warn!(error = %e, "pty reader IO error");
                    let _ = out_tx.blocking_send(PtyOutput::Eof);
                    break;
                }
            }
        }
    });

    let writer_for_input = Arc::clone(&writer);
    let master_for_input = Arc::clone(&master);

    let mut output_done = false;
    let mut input_done = false;

    loop {
        tokio::select! {
            biased;

            chunk = out_rx.recv(), if !output_done => {
                match chunk {
                    Some(PtyOutput::Bytes(bytes)) => {
                        if ws_tx.send(Message::Binary(bytes.into())).await.is_err() {
                            break;
                        }
                    }
                    Some(PtyOutput::Eof) | None => { output_done = true; }
                }
            }

            msg = ws_rx.next(), if !input_done => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let outcome = handle_client_frame(
                            &text,
                            Arc::clone(&writer_for_input),
                            Arc::clone(&master_for_input),
                        ).await;
                        if matches!(outcome, FrameOutcome::WriteFailed) {
                            warn!("pty write failed — closing bridge");
                            let _ = ws_tx.send(error_frame("pty write failed")).await;
                            input_done = true;
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        let w = Arc::clone(&writer_for_input);
                        let res = tokio::task::spawn_blocking(move || {
                            w.lock().write_all(&bytes)
                        }).await;
                        if !matches!(res, Ok(Ok(()))) {
                            warn!("pty write failed (binary)");
                            input_done = true;
                        }
                    }
                    Some(Ok(Message::Ping(p))) => {
                        let _ = ws_tx.send(Message::Pong(p)).await;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        debug!(error = %e, "ws recv error");
                        input_done = true;
                    }
                    None => { input_done = true; }
                }
            }
        }

        if output_done && input_done {
            break;
        }
    }

    let _ = child.kill();
    let reap_result = tokio::task::spawn_blocking(move || {
        let deadline = Instant::now() + CHILD_REAP_TIMEOUT;
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                return Ok(status);
            }
            if Instant::now() > deadline {
                return Err(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    })
    .await;
    match reap_result {
        Ok(Ok(status)) => {
            info!(
                ?status,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "bridge closed"
            );
        }
        Ok(Err(())) => {
            warn!(
                elapsed_ms = started.elapsed().as_millis() as u64,
                "child did not reap within deadline — leaving zombie"
            );
        }
        Err(e) => {
            warn!(error = %e, "child reap task panicked");
        }
    }
    let _ = ws_tx.close().await;
}

enum PtyOutput {
    Bytes(Vec<u8>),
    Eof,
}

enum FrameOutcome {
    Ok,
    WriteFailed,
    Ignored,
}

async fn handle_client_frame(
    text: &str,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
) -> FrameOutcome {
    let frame: ClientFrame = match serde_json::from_str(text) {
        Ok(f) => f,
        Err(_) => return FrameOutcome::Ignored,
    };
    match frame {
        ClientFrame::Input { data } => {
            let bytes = data.into_bytes();
            let res = tokio::task::spawn_blocking(move || writer.lock().write_all(&bytes)).await;
            match res {
                Ok(Ok(())) => FrameOutcome::Ok,
                _ => FrameOutcome::WriteFailed,
            }
        }
        ClientFrame::Resize { cols, rows } => {
            let res = master.lock().resize(PtySize {
                cols,
                rows,
                pixel_width: 0,
                pixel_height: 0,
            });
            if let Err(e) = res {
                debug!(error = %e, cols, rows, "resize failed");
            }
            FrameOutcome::Ok
        }
    }
}

fn error_frame(msg: &str) -> Message {
    Message::Text(format!("\r\n\x1b[31m{msg}\x1b[0m\r\n").into())
}

fn cwd_under_home(path: &std::path::Path) -> bool {
    let Some(home) = dirs::home_dir() else {
        return true;
    };
    let Ok(canon) = std::fs::canonicalize(path) else {
        return false;
    };
    let Ok(home_canon) = std::fs::canonicalize(&home) else {
        return true;
    };
    canon.starts_with(&home_canon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_frame_parses_input() {
        let v: ClientFrame = serde_json::from_str(r#"{"type":"input","data":"hi"}"#).unwrap();
        match v {
            ClientFrame::Input { data } => assert_eq!(data, "hi"),
            _ => panic!("expected input"),
        }
    }

    #[test]
    fn client_frame_parses_resize() {
        let v: ClientFrame =
            serde_json::from_str(r#"{"type":"resize","cols":80,"rows":24}"#).unwrap();
        match v {
            ClientFrame::Resize { cols, rows } => {
                assert_eq!(cols, 80);
                assert_eq!(rows, 24);
            }
            _ => panic!("expected resize"),
        }
    }

    #[test]
    fn client_frame_rejects_unknown_type() {
        assert!(serde_json::from_str::<ClientFrame>(r#"{"type":"bogus"}"#).is_err());
    }

    #[test]
    fn client_frame_rejects_missing_fields() {
        assert!(serde_json::from_str::<ClientFrame>(r#"{"type":"input"}"#).is_err());
        assert!(serde_json::from_str::<ClientFrame>(r#"{"type":"resize","cols":80}"#).is_err());
    }

    #[test]
    fn env_allowlist_contains_minimum_set() {
        for key in ["TERM", "PATH", "HOME"] {
            assert!(ENV_ALLOWLIST.contains(&key), "missing {key}");
        }
    }
}
