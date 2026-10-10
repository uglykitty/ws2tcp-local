//! The control socket: while the proxy runs, `--control PATH` makes it answer on a Unix socket
//! with a JSON snapshot of its HTTP/3 connections. `ws2tcp-local netstat` reads that.

use std::path::PathBuf;

use anyhow::Result;

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
pub fn serve(path: PathBuf) -> Result<ControlSocket> {
    use std::os::unix::{fs::PermissionsExt, net::UnixStream};

    use anyhow::{Context, anyhow};
    use tokio::{io::AsyncWriteExt, net::UnixListener};
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
            tokio::spawn(async move {
                let snapshot = ws2tcp_local_core::http3_snapshot().await;
                let result = match serde_json::to_vec(&snapshot) {
                    Ok(json) => stream.write_all(&json).await,
                    Err(err) => Err(std::io::Error::other(err)),
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
pub fn serve(_path: PathBuf) -> Result<ControlSocket> {
    Err(anyhow::anyhow!(
        "--control is only supported on Linux and other Unix systems"
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use tokio::{io::AsyncReadExt, net::UnixStream};
    use ws2tcp_local_core::Http3Snapshot;

    use super::*;

    fn socket_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ws2tcp-local-{name}-{}.sock", std::process::id()))
    }

    #[tokio::test]
    async fn serves_a_snapshot_and_cleans_up() {
        let path = socket_path("serve");
        let control = serve(path.clone()).unwrap();
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
        let control = serve(path.clone()).unwrap();

        assert!(serve(path.clone()).is_err());
        drop(control);
    }
}
