# ws2tcp-local

[English](README.md)

`ws2tcp-local` 是一个本地 HTTP 代理客户端，用于配合
`ws2tcp-router` 使用。

它接收浏览器或其他客户端的本地代理请求，并为每个目标 TCP 地址通过
WebSocket 连接远端 router。它同时支持 HTTP `CONNECT` 隧道请求和普通
`http://` 代理请求。

```text
browser -> ws2tcp-local -> ws://gateway/tcp:<host>:<port> -> ws2tcp-router -> <host>:<port>
```

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
cargo run -- --listen 127.0.0.1:8000 --gateway wss://example.com/websocat --basic-auth user:pass
```

也可以使用环境变量：

```bash
WS2TCP_LOCAL_BASIC_AUTH=user:pass cargo run -- --gateway wss://example.com/websocat
```

## 配置文件

支持 TOML 配置文件：

```toml
listen = "127.0.0.1:8000"
gateway = "wss://example.com/router"
buffer_size = 16384
log_level = "ws2tcp_local=info"
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
```

## 许可证

MIT。见 [`LICENSE`](LICENSE)。
