use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info};

use crate::{gateway::Gateway, http_proxy::read_proxy_request};

#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub(crate) gateway: Gateway,
    pub(crate) basic_auth: Option<String>,
    pub(crate) buffer_size: usize,
}

pub(crate) async fn handle_client(
    mut client: TcpStream,
    peer_addr: SocketAddr,
    config: Arc<Config>,
) -> Result<()> {
    let request = match read_proxy_request(&mut client).await {
        Ok(request) => request,
        Err(err) => {
            let _ = write_http_error(&mut client, "400 Bad Request").await;
            return Err(err);
        }
    };
    let ws_url = config.gateway.target_url(request.authority());

    info!(%peer_addr, target = %request.authority(), gateway = %ws_url, kind = request.log_kind(), "proxying request");

    let mut ws_request = ws_url
        .as_str()
        .into_client_request()
        .with_context(|| format!("failed to build websocket request for {ws_url}"))?;
    if let Some(basic_auth) = &config.basic_auth {
        ws_request.headers_mut().insert(
            "authorization",
            basic_auth
                .parse()
                .context("failed to build Basic authorization header")?,
        );
    }

    let (websocket, _) = match connect_async(ws_request).await {
        Ok(parts) => parts,
        Err(err) => {
            let _ = write_http_error(&mut client, "502 Bad Gateway").await;
            return Err(err).with_context(|| format!("failed to connect gateway {ws_url}"));
        }
    };

    let is_connect = request.is_connect();
    let initial_client_bytes = request.initial_client_bytes();

    if is_connect {
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .context("write CONNECT success response failed")?;
    }

    proxy(client, websocket, initial_client_bytes, config.buffer_size).await
}

async fn write_http_error(client: &mut TcpStream, status: &str) -> Result<()> {
    let response = format!("HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
    client
        .write_all(response.as_bytes())
        .await
        .context("write HTTP error response failed")
}

async fn proxy(
    client: TcpStream,
    websocket: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    initial_client_bytes: Vec<u8>,
    buffer_size: usize,
) -> Result<()> {
    let (mut ws_writer, mut ws_reader) = websocket.split();
    let (mut client_reader, mut client_writer) = client.into_split();
    let mut client_buffer = vec![0_u8; buffer_size];

    if !initial_client_bytes.is_empty() {
        ws_writer
            .send(Message::Binary(initial_client_bytes.into()))
            .await
            .context("send buffered client bytes to websocket failed")?;
    }

    loop {
        tokio::select! {
            read_result = client_reader.read(&mut client_buffer) => {
                let n = read_result.context("read client failed")?;
                if n == 0 {
                    let _ = ws_writer.send(Message::Close(None)).await;
                    break;
                }

                ws_writer
                    .send(Message::Binary(client_buffer[..n].to_vec().into()))
                    .await
                    .context("send client bytes to websocket failed")?;
            }
            message = ws_reader.next() => {
                match message {
                    Some(Ok(Message::Binary(bytes))) => {
                        client_writer.write_all(&bytes).await.context("write websocket binary frame to client failed")?;
                    }
                    Some(Ok(Message::Text(text))) => {
                        client_writer.write_all(text.as_bytes()).await.context("write websocket text frame to client failed")?;
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        ws_writer.send(Message::Pong(payload)).await.context("send websocket pong failed")?;
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Frame(_))) => {}
                    Some(Ok(Message::Close(frame))) => {
                        debug!(?frame, "websocket closed");
                        client_writer.shutdown().await.context("shutdown client writer failed")?;
                        break;
                    }
                    Some(Err(err)) => return Err(err).context("read websocket frame failed"),
                    None => {
                        client_writer.shutdown().await.context("shutdown client writer failed")?;
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
