# ws2tcp-local

[English](README.md)

`ws2tcp-local` 是一个本地 HTTP 代理客户端，用于配合
`ws2tcp-router` 使用。

它接收浏览器或其他客户端的本地代理请求。自动代理模式下，它使用内置 gfwlist
域名规则为每个目标 TCP 地址选择路由：命中规则的域名会通过远端 WebSocket
router，未命中的域名会本地直连。全局代理模式下，所有请求都会通过远端
WebSocket router，且不会下载 gfwlist。它同时支持 HTTP `CONNECT` 隧道请求和
普通 `http://` 代理请求。

```text
命中规则: browser -> ws2tcp-local -> ws://gateway/tcp:<host>:<port> -> ws2tcp-router -> <host>:<port>
未命中:   browser -> ws2tcp-local -> <host>:<port>
```

启动时，`ws2tcp-local` 会检查并解析下面原始 URL 的 gfwlist：

```text
https://gitlab.com/gfwlist/gfwlist/raw/master/gfwlist.txt
```

该 URL 硬编码在程序中。下载后的 `gfwlist.txt` 会缓存到当前平台的缓存目录：

- Linux 和其他类 Unix 系统：`$XDG_CACHE_HOME/ws2tcp-local/gfwlist.txt`；未设置
  `XDG_CACHE_HOME` 时使用 `~/.cache/ws2tcp-local/gfwlist.txt`。
- macOS：`~/Library/Caches/ws2tcp-local/gfwlist.txt`。
- Windows：`%LOCALAPPDATA%\ws2tcp-local\gfwlist.txt`。

后续启动时，`ws2tcp-local` 会比较本地缓存文件时间和远端 `Last-Modified` 时间，只有
不一致时才重新下载。如果下载或解析失败，`ws2tcp-local` 会回退为所有域名都通过
WebSocket gateway。也可以通过 TOML 配置文件合并自定义域名规则文件。将代理模式设为
`global` 时会跳过规则加载，并代理所有请求。

## 构建

```bash
cargo build --release
```

## 运行

```bash
cargo run -- --listen 127.0.0.1:8000 --gateway ws://1.2.3.4:8000
```

然后将浏览器或系统代理设置为 HTTP 代理 `127.0.0.1:8000`。

如果远端 router 需要 HTTP Basic 认证：

```bash
cargo run -- --listen 127.0.0.1:8000 --gateway wss://example.com --basic-auth user:pass
```

也可以使用环境变量：

```bash
WS2TCP_LOCAL_BASIC_AUTH=user:pass cargo run -- --gateway wss://example.com
```

`wss://` gateway 也受支持：

```bash
cargo run -- --listen 127.0.0.1:8000 --gateway wss://example.com
```

直连 `ws2tcp-router` 时，gateway URL 不应包含路径前缀：
`ws2tcp-local` 会在后面追加 `/tcp:<host>:<port>`，而
`ws2tcp-router` 要求最终的 WebSocket 请求路径以 `/tcp:` 开头。

只有当前面有反向代理，并且反向代理会在转发 WebSocket upgrade 请求前剥离
路径前缀时，才使用 `wss://example.com/router` 这类 gateway。此时
`ws2tcp-local` 会连接 `/router/tcp:<host>:<port>`，反向代理必须把它转发为
`/tcp:<host>:<port>` 给 `ws2tcp-router`。

## 配置文件

支持 TOML 配置文件：

```toml
listen = "127.0.0.1:8000"
gateway = "wss://example.com"
buffer_size = 16384
log_level = "ws2tcp_local=info"
proxy_mode = "global"
verify_server_certificate = false
custom_domain_rules = "custom-domains.txt"
```

启动时指定配置文件：

```bash
cargo run -- --config ws2tcp-local.toml
```

命令行参数会覆盖配置文件中的值：

```bash
cargo run -- --config ws2tcp-local.toml --listen 127.0.0.1:9000
```

示例配置文件见 [`examples/ws2tcp-local.toml`](examples/ws2tcp-local.toml)。

自定义域名规则文件每行一个 Squid `dstdomain` 条目，空行和 `#` 注释会被忽略：

```text
# One Squid dstdomain entry per line.
.paypal.com
.paypalobjects.com
.googleadservices.com
```

相对路径形式的 `custom_domain_rules` 会按配置文件所在目录解析。

也可以直接通过命令行参数指定同一个文件：

```bash
cargo run -- --gateway wss://example.com --custom-domain-rules custom-domains.txt
```

代理模式也可以通过命令行指定。`global` 是默认值，会将所有请求通过 gateway，
并跳过 gfwlist 下载。使用 `auto` 可以加载规则，并让未命中域名直连：

```bash
cargo run -- --gateway wss://example.com --proxy-mode global
```

对于 `wss://` gateway，默认不校验 TLS 服务器证书，因此自签名
`ws2tcp-router` 证书无需额外配置即可使用。程序会在这种模式下输出警告。
如果需要普通 TLS 服务器证书校验，可以显式开启：

```bash
cargo run -- --gateway wss://example.com --verify-server-certificate
```

或在 TOML 配置文件中设置：

```toml
verify_server_certificate = true
```

## 参数

```text
--config <PATH>        TOML 配置文件路径。命令行参数会覆盖配置文件值
--listen <ADDR>        本地代理监听地址。默认值：127.0.0.1:8000
--gateway <URL>        ws:// 或 wss:// ws2tcp-router 基础 URL。
                       除非由 --config 提供，否则必填
--basic-auth <USER:PASS>
                       远端 WebSocket 网关的 HTTP Basic 认证信息。
                       未提供时会回退到 WS2TCP_LOCAL_BASIC_AUTH
--buffer-size <BYTES>  TCP 读取缓冲区大小。默认值：16384
--log-level <FILTER>   日志过滤器，会覆盖 RUST_LOG。例如：ws2tcp_local=debug
--custom-domain-rules <PATH>
                       自定义域名规则文件，每行一个 Squid dstdomain 条目
--proxy-mode <MODE>    代理模式：auto 或 global。默认值：global
--verify-server-certificate
                       校验远端 WebSocket gateway 的 TLS 服务器证书。
                       默认：不校验
```

## 许可证

MIT。见 [`LICENSE`](LICENSE)。
