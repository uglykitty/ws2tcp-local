# Changelog

## 0.2.0 - 2026-09-20

### Added

- `--auth-mode <token|basic>` (and `auth_mode` in the config file) chooses how the CLI
  authenticates to the gateway, one method at a time. `token`, the default, needs a
  `ws2tcp-router` with token authentication: no health check is sent, the CLI logs in once
  with the `--basic-auth` credentials, opens tunnels with a short-lived access token that
  it renews by itself in the background, and never falls back to Basic Auth (a failed
  login fails startup). `basic` is what the CLI always did, a startup health check and
  Basic Auth on every connection; it is kept for compatibility and logs a warning that it
  will be phased out. Requires the matching `ws2tcp-local-core`.

### Changed

- **The default authentication is now `token`.** Against a `ws2tcp-router` that has no
  token authentication (any version before token authentication was added), startup now
  fails with `gateway token login failed`; pass `--auth-mode basic` (or set
  `auth_mode = "basic"`) to keep the old behavior.
- **Without credentials nothing is sent at startup any more.** No `--basic-auth` means
  authentication is not enabled, so the health check is skipped and the proxy starts
  right away. Before, the health check ran and reported an unreachable gateway or a
  router that requires credentials; now those show at the first connection.
- The `X-Ws2tcp-Token` header from the health check is no longer sent on proxied
  connections (the router ignored it).

## 0.1.20 - 2026-09-19

### Added

- On startup the CLI now checks the gateway (a websocket handshake on the
  gateway root, answered by `ws2tcp-router`'s health check) before listening.
  If the gateway rejects the Basic Auth credentials (`401`), or none were
  configured but the gateway needs them, it logs what to fix and exits with
  status 1. Any other failure (unreachable, timeout, a router without the
  health check) also exits with status 1. Requires `ws2tcp-local-core` 0.1.9
  and a `ws2tcp-router` with the `/` health check (0.1.17 or later).
- The token returned by that health check (`X-Ws2tcp-Token` response header)
  is now sent as the same header on every proxied connection, together with
  the Basic Auth credentials. `ws2tcp-router` does not verify it yet.

## 0.1.19 - 2026-09-19

### Changed

- The CLI now sends `User-Agent: ws2tcp-local/<version>` on the gateway
  websocket handshake (so `ws2tcp-router` can log it), via `ws2tcp-local-core`'s
  new `Settings::add_header`, instead of `client_label` on gfwlist requests.
  Requires `ws2tcp-local-core` 0.1.8.

## 0.1.18 - 2026-09-17

### Added

- The CLI now identifies itself as `cli/0.1.18` in the `User-Agent` sent
  with gfwlist HTTP requests, via `ws2tcp-local-core`'s new
  `Settings.client_label`. Requires `ws2tcp-local-core` 0.1.7.

## 0.1.17 - 2026-09-09

### Added

- Added `--socks-listen` / `socks_listen` for an optional local SOCKS5
  (`socks5h`) proxy, sharing the same gateway, routing rules, and proxy mode
  as the existing HTTP proxy. Pass `--socks-listen` without a value to bind
  the conventional `127.0.0.1:1080`, or give it an address to override.
  Omitted entirely, no SOCKS5 listener is started. Requires
  `ws2tcp-local-core` 0.1.6.

## 0.1.16 - 2026-09-07

### Fixed

- Stopped emitting a duplicate timestamp in `journalctl` output when running
  under systemd: log lines no longer include our own timestamp when
  `JOURNAL_STREAM` is set, since journald already records one per entry.
  Interactive terminal runs are unaffected.

### Changed

- Moved logging setup (`init_logging`) from `ws2tcp-local-core` into this
  CLI, since a shared library used by multiple frontends (CLI, FFI, GUI)
  shouldn't install a process-global `tracing` subscriber on their behalf.
  Updated `ws2tcp-local-core` to 0.1.5.

## 0.1.15 - 2026-09-05

### Changed

- Updated `ws2tcp-local-core` to 0.1.4. Gfwlist is now downloaded from the
  primary mirror
  `https://wangguofang.net/raw.githubusercontent.com/gfwlist/gfwlist/refs/heads/master/gfwlist.txt`
  first, falling back to GitLab when the primary URL is unreachable.

## 0.1.14 - 2026-08-24

### Changed

- Replaced `--verify-server-certificate` with the curl-style `--insecure`
  option and enabled TLS server certificate verification by default.
- Renamed the TOML setting to `insecure` and documented all command-line
  defaults in `--help`.
- Updated `ws2tcp-local-core` to 0.1.3.

## 0.1.12 - 2026-07-27

### Fixed

- Include the embedded example configuration in Docker image builds.

## 0.1.11 - 2026-07-16

### Added

- Added `--generate-config` to print a TOML configuration template to stdout
  and exit without starting the proxy.

### Fixed

- Updated the documented container volume mount paths for broader compatibility.

## 0.1.10 - 2026-07-14

### Changed

- Updated `ws2tcp-local-core` to 0.1.2.
- Added automatic fallback to an in-memory gfwlist cache when the platform disk
  cache is not readable and writable.

## 0.1.9 - 2026-07-14

### Changed

- Changed the default local proxy listen address from `127.0.0.1:8000` to
  `127.0.0.1:3128`.
- Changed the default proxy mode from `global` to `auto`.
- Added Podman usage instructions using the published container image.
- Updated the container image to expose port 3128 and load a mounted TOML
  configuration from `/etc/ws2tcp-local/ws2tcp-local.toml` by default.

### Security

- Added non-blocking warnings when Basic Auth credentials are supplied through
  command-line arguments or the process environment, without logging the
  credentials themselves.

## 0.1.8 - 2026-07-13

### Changed

- Build the container image from the checked-out `ws2tcp-local` and `ws2tcp-local-core` sources with locked dependencies.
- Track `Cargo.lock` for reproducible application and container builds.
- Align the local Podman build context with the GitHub Actions checkout layout.

### Fixed

- Corrected the builder artifact path used by the final container image stage.
- Fixed the local container build script's shebang and made it independent of the caller's working directory.

## 0.1.5 - 2026-07-08

### Changed

- Changed auto proxy rule loading from startup-only loading to periodic hot reload.
- Added configurable rule refresh interval with `--rule-refresh-interval-secs` and `rule_refresh_interval_secs`; the default is 60 seconds.
- Kept gfwlist downloads conditional on remote `Last-Modified` changes so unchanged lists continue to use the local cache.
- Added hot reload for custom domain rules using the custom rules file modification time.
- Changed auto mode fallback behavior to route directly when rules are unavailable, while still proxying only hosts matched by loaded rules.
- Replaced active routing rules atomically on successful refresh and kept the previous active rules when refresh fails.
- Updated English and Chinese documentation plus the example TOML configuration for the new rule refresh behavior.
