//! The control socket: while the proxy runs, `--control PATH` makes it listen on a Unix socket.
//! A client sends one command line and reads the reply until the connection closes:
//!
//! - `snapshot` (also an empty request): a JSON snapshot of the HTTP/3 connections, which
//!   `ws2tcp-local netstat` prints.
//! - `get [KEY]` and `set KEY VALUE`: read or change a setting of the running proxy. `mode` is
//!   `auto` or `global`, `http3` is `off`, `on` (HTTP/3 first, TCP as the fallback) or `only`.
//! - `reset-quic`: drop the cached HTTP/3 connection, so the next tunnel dials a new one.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use tokio::sync::mpsc;
use ws2tcp_local_core::{Http3Mode, ProxyMode, Settings};

/// The settings of the running proxy that can be read and changed from the control socket.
pub struct Controller {
    mode: std::sync::Mutex<ProxyMode>,
    mode_updates: mpsc::UnboundedSender<ProxyMode>,
    http3: std::sync::Mutex<Http3Mode>,
    http3_updates: mpsc::UnboundedSender<Http3Mode>,
    /// Why HTTP/3 cannot be turned on with this gateway and upstream proxy, if it cannot.
    http3_blocked: Option<&'static str>,
}

impl Controller {
    pub fn new(
        settings: &Settings,
        mode_updates: mpsc::UnboundedSender<ProxyMode>,
        http3_updates: mpsc::UnboundedSender<Http3Mode>,
    ) -> Self {
        Self {
            mode: std::sync::Mutex::new(settings.proxy_mode),
            mode_updates,
            http3: std::sync::Mutex::new(ws2tcp_local_core::http3_mode(settings)),
            http3_updates,
            http3_blocked: ws2tcp_local_core::http3_unusable_for(
                &settings.gateway,
                settings.upstream_proxy.is_some(),
            ),
        }
    }

    fn get(&self, key: &str) -> Result<String, String> {
        match key {
            "mode" => Ok(mode_name(*self.mode.lock().unwrap()).to_owned()),
            "http3" => Ok(http3_name(*self.http3.lock().unwrap()).to_owned()),
            _ => Err(unknown_key(key)),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "mode" => {
                let mode = match value {
                    "auto" => ProxyMode::Auto,
                    "global" => ProxyMode::Global,
                    _ => return Err(format!("mode is auto or global, not {value:?}")),
                };
                let mut current = self.mode.lock().unwrap();
                if *current != mode {
                    self.mode_updates
                        .send(mode)
                        .map_err(|_| "the proxy is shutting down".to_owned())?;
                    *current = mode;
                    tracing::info!(mode = value, "control: proxy mode changed");
                }
                Ok(())
            }
            "http3" => {
                let mode = match value {
                    "off" => Http3Mode::Off,
                    "on" => Http3Mode::Preferred,
                    "only" => Http3Mode::Only,
                    _ => return Err(format!("http3 is off, on or only, not {value:?}")),
                };
                if let Some(reason) = self.http3_blocked
                    && mode != Http3Mode::Off
                {
                    return Err(format!("HTTP/3 cannot be used because {reason}"));
                }
                let mut current = self.http3.lock().unwrap();
                if *current != mode {
                    self.http3_updates
                        .send(mode)
                        .map_err(|_| "the proxy is shutting down".to_owned())?;
                    *current = mode;
                    tracing::info!(http3 = value, "control: HTTP/3 mode changed");
                }
                Ok(())
            }
            _ => Err(unknown_key(key)),
        }
    }

    /// Answer a `get` or `set` command line.
    fn config_command(&self, words: &[&str]) -> Result<String, String> {
        match words {
            ["get"] => Ok(format!(
                "mode: {}\nhttp3: {}\n",
                self.get("mode")?,
                self.get("http3")?
            )),
            ["get", key] => Ok(format!("{}\n", self.get(key)?)),
            ["set", key, value] => {
                self.set(key, value)?;
                Ok("ok\n".to_owned())
            }
            _ => Err("usage: get [KEY] | set KEY VALUE".to_owned()),
        }
    }

    pub async fn reset_quic(&self) {
        tracing::info!("control: the next tunnel will dial a new QUIC connection");
        ws2tcp_local_core::reset_sessions().await;
    }
}

fn unknown_key(key: &str) -> String {
    format!("unknown setting {key:?}; the settings are mode and http3")
}

/// Removes the socket file when dropped.
pub struct ControlSocket {
    path: PathBuf,
}

impl Drop for ControlSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
pub fn serve(path: PathBuf, controller: Arc<Controller>) -> Result<ControlSocket> {
    use std::{
        os::unix::{fs::PermissionsExt, net::UnixStream},
        time::Duration,
    };

    use anyhow::{Context, anyhow};
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::UnixListener,
    };
    use tracing::{debug, info, warn};

    // A socket file left by a crashed proxy is replaced; one that still answers is not.
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            return Err(anyhow!(
                "another proxy already listens on the control socket {}",
                path.display()
            ));
        }
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove the stale socket {}", path.display()))?;
    }
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("failed to bind the control socket {}", path.display()))?;
    // Connecting needs write permission on the socket, so other users are locked out even before
    // this call, whatever the umask.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to restrict the control socket {}", path.display()))?;
    info!(path = %path.display(), "control socket ready");

    tokio::spawn(async move {
        loop {
            let mut stream = match listener.accept().await {
                Ok((stream, _)) => stream,
                Err(err) => {
                    warn!(error = %err, "control socket accept failed");
                    continue;
                }
            };
            let controller = controller.clone();
            tokio::spawn(async move {
                // A client that sends nothing is treated as asking for the snapshot.
                let mut line = String::new();
                let mut reader = BufReader::new(&mut stream);
                let _ =
                    tokio::time::timeout(Duration::from_secs(1), reader.read_line(&mut line)).await;
                let result = match line.trim() {
                    "" | "snapshot" => {
                        let snapshot = ws2tcp_local_core::http3_snapshot().await;
                        serde_json::to_vec(&snapshot).map_err(std::io::Error::other)
                    }
                    "reset-quic" => {
                        controller.reset_quic().await;
                        Ok(b"ok\n".to_vec())
                    }
                    command => {
                        let words: Vec<&str> = command.split_whitespace().collect();
                        Ok(match controller.config_command(&words) {
                            Ok(reply) => reply,
                            Err(err) => format!("error: {err}\n"),
                        }
                        .into_bytes())
                    }
                };
                let result = match result {
                    Ok(reply) => stream.write_all(&reply).await,
                    Err(err) => Err(err),
                };
                if let Err(err) = result {
                    debug!(error = %err, "failed to answer on the control socket");
                }
                let _ = stream.shutdown().await;
            });
        }
    });

    Ok(ControlSocket { path })
}

#[cfg(not(unix))]
pub fn serve(_path: PathBuf, _controller: Arc<Controller>) -> Result<ControlSocket> {
    Err(anyhow::anyhow!(
        "--control is only supported on Linux and other Unix systems"
    ))
}

fn mode_name(mode: ProxyMode) -> &'static str {
    match mode {
        ProxyMode::Auto => "auto",
        ProxyMode::Global => "global",
    }
}

fn http3_name(mode: Http3Mode) -> &'static str {
    match mode {
        Http3Mode::Off => "off",
        Http3Mode::Preferred => "on",
        Http3Mode::Only => "only",
    }
}

/// Send `command` to the proxy listening on `path` and return its whole reply.
#[cfg(unix)]
pub async fn request(path: &Path, command: &str) -> Result<String> {
    use anyhow::Context;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::UnixStream,
    };

    let mut stream = UnixStream::connect(path).await.with_context(|| {
        format!(
            "cannot reach the proxy on {}; is it running with --control?",
            path.display()
        )
    })?;
    stream.write_all(format!("{command}\n").as_bytes()).await?;
    let mut body = String::new();
    stream.read_to_string(&mut body).await?;
    Ok(body)
}

#[cfg(not(unix))]
pub async fn request(_path: &Path, _command: &str) -> Result<String> {
    Err(anyhow::anyhow!(
        "control commands are only supported on Linux and other Unix systems"
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use tokio::{io::AsyncReadExt, net::UnixStream};
    use ws2tcp_local_core::{Http3Snapshot, SettingsOverrides};

    use super::*;

    fn controller(
        mode_tx: mpsc::UnboundedSender<ProxyMode>,
        http3_tx: mpsc::UnboundedSender<Http3Mode>,
    ) -> Arc<Controller> {
        let settings = Settings::resolve(SettingsOverrides {
            gateway: Some("wss://example.com/tunnel".to_owned()),
            ..Default::default()
        })
        .unwrap();
        Arc::new(Controller::new(&settings, mode_tx, http3_tx))
    }

    fn socket_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ws2tcp-local-{name}-{}.sock", std::process::id()))
    }

    #[tokio::test]
    async fn serves_a_snapshot_and_cleans_up() {
        let path = socket_path("serve");
        let (mode_tx, _mode_rx) = mpsc::unbounded_channel();
        let (http3_tx, _http3_rx) = mpsc::unbounded_channel();
        let control = serve(path.clone(), controller(mode_tx, http3_tx)).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);

        let mut stream = UnixStream::connect(&path).await.unwrap();
        let mut body = Vec::new();
        stream.read_to_end(&mut body).await.unwrap();
        let snapshot: Http3Snapshot = serde_json::from_slice(&body).unwrap();
        assert!(snapshot.connections.is_empty());

        drop(control);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn replaces_a_stale_socket_but_not_a_live_one() {
        let path = socket_path("stale");
        // A bound socket whose listener is gone, as a crashed proxy leaves it.
        drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
        let (mode_tx, _mode_rx) = mpsc::unbounded_channel();
        let (http3_tx, _http3_rx) = mpsc::unbounded_channel();
        let controller = controller(mode_tx, http3_tx);
        let control = serve(path.clone(), controller.clone()).unwrap();

        assert!(serve(path.clone(), controller).is_err());
        drop(control);
    }

    #[tokio::test]
    async fn gets_and_sets_settings() {
        let path = socket_path("config");
        let (mode_tx, mut mode_rx) = mpsc::unbounded_channel();
        let (http3_tx, mut http3_rx) = mpsc::unbounded_channel();
        let control = serve(path.clone(), controller(mode_tx, http3_tx)).unwrap();

        assert_eq!(request(&path, "get mode").await.unwrap(), "auto\n");
        assert_eq!(
            request(&path, "get").await.unwrap(),
            "mode: auto\nhttp3: off\n"
        );
        assert_eq!(request(&path, "set mode global").await.unwrap(), "ok\n");
        assert_eq!(mode_rx.recv().await, Some(ProxyMode::Global));
        assert_eq!(request(&path, "get mode").await.unwrap(), "global\n");
        assert_eq!(request(&path, "set http3 on").await.unwrap(), "ok\n");
        assert_eq!(http3_rx.recv().await, Some(Http3Mode::Preferred));
        assert_eq!(request(&path, "get http3").await.unwrap(), "on\n");
        assert!(
            request(&path, "set mode bogus")
                .await
                .unwrap()
                .starts_with("error")
        );
        assert!(
            request(&path, "get bogus")
                .await
                .unwrap()
                .starts_with("error")
        );
        drop(control);
    }

    #[tokio::test]
    async fn refuses_http3_without_a_wss_gateway() {
        let path = socket_path("blocked");
        let (mode_tx, _mode_rx) = mpsc::unbounded_channel();
        let (http3_tx, _http3_rx) = mpsc::unbounded_channel();
        let settings = Settings::resolve(SettingsOverrides {
            gateway: Some("ws://example.com/tunnel".to_owned()),
            ..Default::default()
        })
        .unwrap();
        let controller = Arc::new(Controller::new(&settings, mode_tx, http3_tx));
        let control = serve(path.clone(), controller).unwrap();

        assert!(
            request(&path, "set http3 on")
                .await
                .unwrap()
                .starts_with("error")
        );
        assert_eq!(request(&path, "set http3 off").await.unwrap(), "ok\n");
        drop(control);
    }
}
