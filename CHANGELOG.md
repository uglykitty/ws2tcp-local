# Changelog

## Unreleased

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
