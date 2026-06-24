# ws2tcp-local

[中文](README.zh-CN.md)

`ws2tcp-local` is a local HTTP proxy client for `ws2tcp-router`.

It accepts local browser proxy connections and routes each requested TCP target
in auto proxy mode with a built-in gfwlist domain set. Matched domains go
through the remote WebSocket router, and unmatched domains connect directly. In
global proxy mode, every request goes through the remote WebSocket router and
`ws2tcp-local` does not download gfwlist. It supports both HTTP `CONNECT`
tunnels and ordinary `http://` proxy requests.

```text
matched:   browser -> ws2tcp-local -> ws://gateway/tcp:<host>:<port> -> ws2tcp-router -> <host>:<port>
unmatched: browser -> ws2tcp-local -> <host>:<port>
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

On startup, `ws2tcp-local` downloads and parses the original gfwlist from:

```text
https://gitlab.com/gfwlist/gfwlist/raw/master/gfwlist.txt
```

The URL is built into the program. If the download or parsing step fails,
`ws2tcp-local` falls back to sending all domains through the WebSocket gateway.
You can also merge a custom domain rules file from the TOML configuration.
Set proxy mode to `global` to skip rule loading and proxy every request.

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
cargo run -- --listen 127.0.0.1:8000 --gateway wss://example.com --basic-auth user:pass
```

Or use an environment variable:

```bash
WS2TCP_LOCAL_BASIC_AUTH=user:pass cargo run -- --gateway wss://example.com
```

`wss://` gateways are supported:

```bash
cargo run -- --listen 127.0.0.1:8000 --gateway wss://example.com
```

When connecting directly to `ws2tcp-router`, the gateway URL should not include a
path prefix: `ws2tcp-local` appends `/tcp:<host>:<port>`, and `ws2tcp-router`
expects the final WebSocket request path to start with `/tcp:`.

Use a gateway path such as `wss://example.com/router` only when a reverse proxy
in front of `ws2tcp-router` strips that prefix before forwarding the WebSocket
upgrade request. In that deployment, `ws2tcp-local` connects to
`/router/tcp:<host>:<port>`, and the reverse proxy must forward it to
`ws2tcp-router` as `/tcp:<host>:<port>`.

Configuration files are also supported:

```toml
listen = "127.0.0.1:8000"
gateway = "wss://example.com"
buffer_size = 16384
log_level = "ws2tcp_local=info"
proxy_mode = "global"
verify_server_certificate = false
custom_domain_rules = "custom-domains.txt"
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

The custom domain rules file uses one Squid `dstdomain` entry per line. Blank
lines and `#` comments are ignored:

```text
# One Squid dstdomain entry per line.
.paypal.com
.paypalobjects.com
.googleadservices.com
```

Relative `custom_domain_rules` paths are resolved from the config file's
directory.

You can also provide the same file directly on the command line:

```bash
cargo run -- --gateway wss://example.com --custom-domain-rules custom-domains.txt
```

Proxy mode can also be set from the command line. `global` is the default and
routes every request through the gateway while skipping gfwlist download.
Use `auto` to load rules and direct-connect unmatched domains:

```bash
cargo run -- --gateway wss://example.com --proxy-mode global
```

For `wss://` gateways, TLS server certificate verification is disabled by
default so self-signed `ws2tcp-router` certificates work without extra setup.
The program logs a warning when running this way. To require normal TLS server
certificate validation, enable it explicitly:

```bash
cargo run -- --gateway wss://example.com --verify-server-certificate
```

Or in the TOML configuration:

```toml
verify_server_certificate = true
```

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
--custom-domain-rules <PATH>
                       Custom domain rules file, one Squid dstdomain entry per line
--proxy-mode <MODE>    Proxy mode: auto or global. Default: global
--verify-server-certificate
                       Verify the remote WebSocket gateway TLS certificate.
                       Default: disabled
```

## License

MIT. See [`LICENSE`](LICENSE).
