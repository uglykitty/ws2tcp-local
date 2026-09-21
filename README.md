# ws2tcp-local

[中文](README.zh_CN.md)

`ws2tcp-local` is a local HTTP proxy client for `ws2tcp-router`.

It accepts local browser proxy connections and routes each requested TCP target
in auto proxy mode with a built-in gfwlist domain set. Matched domains go
through the remote WebSocket router, and unmatched domains connect directly. In
global proxy mode, every request goes through the remote WebSocket router and
`ws2tcp-local` does not download gfwlist. It supports both HTTP `CONNECT`
tunnels and ordinary `http://` proxy requests, and can optionally bind a second
listener speaking SOCKS5 (`socks5h`: hostnames are forwarded to the gateway or
direct connection as-is, not resolved locally).

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

In `auto` proxy mode, `ws2tcp-local` checks and parses the original gfwlist from
these built-in URLs at startup, then refreshes it on a configurable interval that
defaults to 60 seconds. The primary URL is tried first, falling back to GitLab
when it is unreachable:

```text
https://wangguofang.net/raw.githubusercontent.com/gfwlist/gfwlist/refs/heads/master/gfwlist.txt
https://gitlab.com/gfwlist/gfwlist/raw/master/gfwlist.txt
```

The URLs are built into the program. The downloaded `gfwlist.txt` is cached in
the platform cache directory:

- Linux and other Unix-like systems: `$XDG_CACHE_HOME/ws2tcp-local/gfwlist.txt`,
  or `~/.cache/ws2tcp-local/gfwlist.txt` when `XDG_CACHE_HOME` is not set.
- macOS: `~/Library/Caches/ws2tcp-local/gfwlist.txt`.
- Windows: `%LOCALAPPDATA%\ws2tcp-local\gfwlist.txt`.

After a successful download, `ws2tcp-local` stores the remote `Last-Modified`
time on the cached file. Later startup and refresh checks compare that cached
timestamp with the remote `Last-Modified` timestamp and download again only when
they differ. If loading fails before rules are available, `auto` mode routes
directly by default; only hosts matching the loaded rule set use the WebSocket
gateway. You can also merge a custom domain rules file from the TOML
configuration; in `auto` mode, that file is checked on the same refresh interval
and reloaded only when its modification time changes. Set proxy mode to `global`
to skip rule loading and proxy every request.

## Build

```bash
cargo build --release
```

## Run

```bash
cargo run -- --listen 127.0.0.1:3128 --gateway wss://wangguofang.net/tunnel
```

Then configure Chrome or Firefox to use `127.0.0.1:3128` as an HTTP proxy.

To also expose a SOCKS5 (`socks5h`) proxy, pass `--socks-listen`. Without a
value it binds `127.0.0.1:1080`; give it a value to override the address.
Omit the flag entirely to keep the SOCKS5 listener disabled:

```bash
cargo run -- --listen 127.0.0.1:3128 --socks-listen --gateway wss://wangguofang.net/tunnel
```

The SOCKS5 listener shares the same gateway, routing rules, and proxy mode as
the HTTP listener; only the no-authentication SOCKS5 method is supported.

If the remote router requires HTTP Basic authentication:

```bash
cargo run -- --listen 127.0.0.1:3128 --gateway wss://wangguofang.net/tunnel --basic-auth user:pass
```

Or use an environment variable:

```bash
WS2TCP_LOCAL_BASIC_AUTH=user:pass cargo run -- --gateway wss://wangguofang.net/tunnel
```

`ws2tcp-local` authenticates to the gateway with exactly one method at a time, chosen
with `--auth-mode` (or `auth_mode` in the config file). The default is `token`. Basic Auth
on every connection is kept only for compatibility with routers that have no token
authentication, and will be phased out.

- `token` (the default): needs a `ws2tcp-router` with token authentication, which it has by
  default when it has credentials configured. No health check is sent: the client logs in
  once with the credentials from `--basic-auth` (`POST /auth/token`), and that login is the
  check. It then opens tunnels with a short-lived access token that it renews by itself in
  the background, so the password is not sent on every connection. There is no fallback to
  Basic Auth: if the login fails (wrong credentials, gateway unreachable, or no token
  endpoints), startup fails with the reason. What is given up compared with `basic` is the
  check that the gateway is a `ws2tcp-router` and that a WebSocket upgrade works through it,
  which now shows at the first tunnel.
- `basic` (compatibility only): on startup it checks the gateway with a health check and
  Basic Auth. If the credentials are wrong it prints what to fix and exits with status 1
  instead of starting the proxy; it also exits if the gateway cannot be reached or is a
  `ws2tcp-router` without the `/` health check. Every proxied connection then sends the
  Basic Auth credentials, and a warning at startup reminds that this mode is on its way out.

Without credentials (no `--basic-auth` and no `WS2TCP_LOCAL_BASIC_AUTH`) authentication is
not enabled: nothing is sent at startup in either mode, and the proxy starts right away and
uses the gateway without authentication.

```bash
cargo run -- --gateway wss://wangguofang.net/tunnel --basic-auth user:pass
```

A router that has no token authentication (an older `ws2tcp-router`) needs the compatibility
mode:

```bash
cargo run -- --gateway wss://wangguofang.net/tunnel --basic-auth user:pass --auth-mode basic
```

Behind a reverse proxy, `token` mode needs the same path prefix to also forward plain
HTTP `POST <gateway>/auth/token` and `POST <gateway>/auth/refresh` to the router.

`wss://` gateways are supported:

```bash
cargo run -- --listen 127.0.0.1:3128 --gateway wss://wangguofang.net/tunnel
```

When connecting directly to `ws2tcp-router`, the gateway URL should not include a
path prefix: `ws2tcp-local` appends `/tcp:<host>:<port>`, and `ws2tcp-router`
expects the final WebSocket request path to start with `/tcp:`.

Use a gateway path such as `wss://wangguofang.net/tunnel` only when a reverse proxy
in front of `ws2tcp-router` strips that prefix before forwarding the WebSocket
upgrade request. In that deployment, `ws2tcp-local` connects to
`/tunnel/tcp:<host>:<port>`, and the reverse proxy must forward it to
`ws2tcp-router` as `/tcp:<host>:<port>`.

Configuration files are also supported:

```toml
listen = "127.0.0.1:3128"
# socks_listen = "127.0.0.1:1080"
gateway = "wss://wangguofang.net/tunnel"
# basic_auth = "user:passwd"
buffer_size = 16384
log_level = "ws2tcp_local=info"
proxy_mode = "auto"
insecure = false
custom_domain_rules = "custom-domains.txt"
rule_refresh_interval_secs = 60
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
The same template can be printed to stdout without starting the proxy:

```bash
ws2tcp-local --generate-config > ws2tcp-local.toml
```

## Podman

Build the image from the parent directory, which must contain both the
`ws2tcp-local` and `ws2tcp-local-core` repositories:

```bash
podman build -t ws2tcp-local -f ws2tcp-local/Dockerfile .
```

Create a configuration file for the container. The listener must bind to all
interfaces so it can be reached through the published port:

```toml
listen = "[::]:3128"
gateway = "wss://wangguofang.net/tunnel"
# basic_auth = "user:passwd"
proxy_mode = "auto"
```

The published image is available from GitHub Container Registry. Run it with
Podman and mount the configuration file at the image's default configuration
path. The `:ro` option keeps the configuration read-only inside the container:

```bash
podman run --name ws2tcp-local -t \
  -p 3128:3128 \
  -v "${PWD}/ws2tcp-local.toml:/etc/ws2tcp-local/ws2tcp-local.toml:ro" \
  ghcr.io/uglykitty/ws2tcp-local:latest
```

Use `127.0.0.1:3128` as the HTTP proxy on the host. To mount the configuration
at another path, override the image's default command. Remove the existing
named container first with `podman rm ws2tcp-local`, or choose another name:

```bash
podman run --name ws2tcp-local -t \
  -p 3128:3128 \
  -v "${PWD}/ws2tcp-local.toml:/config/local.toml:ro" \
  ghcr.io/uglykitty/ws2tcp-local:latest --config /config/local.toml
```

The custom domain rules file uses one Squid `dstdomain` entry per line. Blank
lines and `#` comments are ignored:

```text
# One Squid dstdomain entry per line.
.paypal.com
.paypalobjects.com
.googleadservices.com
```

Relative `custom_domain_rules` paths are resolved from the config file's
directory. In `auto` mode, changes to this file are picked up on the next rule
refresh when the file modification time changes.

You can also provide the same file directly on the command line:

```bash
cargo run -- --gateway wss://wangguofang.net/tunnel --custom-domain-rules custom-domains.txt
```

Proxy mode can also be set from the command line. `auto` is the default; it
loads rules and directly connects unmatched domains. Use `global` to route
every request through the gateway while skipping gfwlist download:

```bash
cargo run -- --gateway wss://wangguofang.net/tunnel --proxy-mode global
```

On Unix (Linux and macOS), send `SIGUSR1` to switch the proxy mode of a running
process between `auto` and `global`, for example `kill -USR1 <pid>`. Each
signal toggles the mode, starting from the mode the process was launched with.
Switching to `auto` downloads the rules first and keeps the current mode until
they are loaded; if that fails, unmatched domains are connected directly. The
switch is logged, and it is not written back to the config file. Windows has no
`SIGUSR1`, so this is not available there.

For `wss://` gateways, TLS server certificates are verified by default. To
connect to a gateway with an untrusted certificate, such as a self-signed
certificate used during development, enable insecure mode explicitly:

```bash
cargo run -- --gateway wss://wangguofang.net/tunnel --insecure
```

Or in the TOML configuration:

```toml
insecure = true
```

## Options

```text
--generate-config       Print a TOML configuration template to stdout and exit
--config <PATH>        TOML config file path. CLI arguments override config values
--listen <ADDR>        Local proxy listen address. Default: 127.0.0.1:3128
--socks-listen [<ADDR>]
                       Also bind a local SOCKS5 (socks5h) proxy. Pass without a
                       value to use 127.0.0.1:1080. Omitted, no SOCKS5 listener
                       is started
--gateway <URL>        Base ws:// or wss:// ws2tcp-router URL. Required unless
                       provided by --config
--basic-auth <USER:PASS>
                       HTTP Basic auth credential for the remote WebSocket gateway.
                       Falls back to WS2TCP_LOCAL_BASIC_AUTH when omitted
--auth-mode <MODE>     How to authenticate to the gateway, one method at a time:
                       token (no health check, log in once for an access token)
                       or basic (health check, then Basic Auth on every
                       connection; compatibility only, being phased out).
                       Default: token
--buffer-size <BYTES>  TCP read buffer size. Default: 16384
--log-level <FILTER>   Logging filter, overriding RUST_LOG. Example: ws2tcp_local=debug
--custom-domain-rules <PATH>
                       Custom domain rules file, one Squid dstdomain entry per line
--rule-refresh-interval-secs <SECONDS>
                       Rule list refresh interval in seconds. Default: 60
--proxy-mode <MODE>    Proxy mode: auto or global. Default: auto
--insecure             Skip verification of the remote WebSocket gateway TLS
                       certificate. Default: disabled
```

## License

MIT. See [`LICENSE`](LICENSE).
