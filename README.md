# ws2tcp-local

[中文](README.zh-CN.md)

`ws2tcp-local` is a local HTTP proxy client for `ws2tcp-router`.

It accepts local browser proxy connections and opens a WebSocket tunnel to a
remote router for each requested TCP target. It supports both HTTP `CONNECT`
tunnels and ordinary `http://` proxy requests.

```text
browser -> ws2tcp-local -> ws://gateway/tcp:<host>:<port> -> ws2tcp-router -> <host>:<port>
```

For example, when a browser sends a tunnel request:

```text
CONNECT www.google.com:443 HTTP/1.1
```

`ws2tcp-local` connects to:

```text
ws://1.2.3.4:8000/tcp:www.google.com:443
```

and then forwards bytes in both directions.

For ordinary HTTP proxy requests such as:

```text
GET http://example.com/path HTTP/1.1
```

`ws2tcp-local` connects to `tcp:example.com:80`, rewrites the request to
origin-form, and forwards the response back to the client.

## Build

```bash
cargo build --release
```

## Run

```bash
cargo run -- --listen 127.0.0.1:8000 --gateway ws://1.2.3.4:8000
```

Then configure Chrome or Firefox to use `127.0.0.1:8000` as an HTTP proxy.

If the remote router requires HTTP Basic authentication:

```bash
cargo run -- --listen 127.0.0.1:8000 --gateway wss://example.com/websocat --basic-auth user:pass
```

Or use an environment variable:

```bash
WS2TCP_LOCAL_BASIC_AUTH=user:pass cargo run -- --gateway wss://example.com/websocat
```

`wss://` gateways are supported:

```bash
cargo run -- --listen 127.0.0.1:8000 --gateway wss://example.com/router
```

Configuration files are also supported:

```toml
listen = "127.0.0.1:8000"
gateway = "wss://example.com/router"
buffer_size = 16384
log_level = "ws2tcp_local=info"
```

```bash
cargo run -- --config ws2tcp-local.toml
```

Command-line arguments override values loaded from the config file:

```bash
cargo run -- --config ws2tcp-local.toml --listen 127.0.0.1:9000
```

An example config file is available at
[`examples/ws2tcp-local.toml`](examples/ws2tcp-local.toml).

## Options

```text
--config <PATH>        TOML config file path. CLI arguments override config values
--listen <ADDR>        Local proxy listen address. Default: 127.0.0.1:8000
--gateway <URL>        Base ws:// or wss:// ws2tcp-router URL. Required unless
                       provided by --config
--basic-auth <USER:PASS>
                       HTTP Basic auth credential for the remote WebSocket gateway.
                       Falls back to WS2TCP_LOCAL_BASIC_AUTH when omitted
--buffer-size <BYTES>  TCP read buffer size. Default: 16384
--log-level <FILTER>   Logging filter, overriding RUST_LOG. Example: ws2tcp_local=debug
```

## License

MIT. See [`LICENSE`](LICENSE).
