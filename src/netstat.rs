//! `ws2tcp-local netstat`: prints the HTTP/3 connections of a running proxy.

use std::{
    io::{IsTerminal, Write},
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Result;
use comfy_table::{CellAlignment, Table, presets::NOTHING};
use ws2tcp_local_core::{Http3ConnInfo, Http3Snapshot};

pub async fn run(path: &Path, json: bool, watch: Option<u64>) -> Result<()> {
    let Some(secs) = watch else {
        return print_once(path, json).await;
    };
    let clear = std::io::stdout().is_terminal();
    // The previous snapshot and when it was taken, to turn the byte counters into rates.
    let mut previous: Option<(Http3Snapshot, Instant)> = None;
    loop {
        let body = fetch(path).await?;
        let taken = Instant::now();
        if clear {
            print!("\x1b[2J\x1b[H");
        }
        if json {
            println!("{}", body.trim_end());
        } else {
            let snapshot: Http3Snapshot = serde_json::from_str(&body)?;
            let basis = previous
                .as_ref()
                .map(|(snapshot, at)| (snapshot, taken.duration_since(*at)));
            print!("{}", render(&snapshot, Some(basis)));
            previous = Some((snapshot, taken));
        }
        // Frames must reach a pipe or file as they are printed, not when the buffer fills.
        std::io::stdout().flush()?;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(secs.max(1))) => {}
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
    }
}

async fn print_once(path: &Path, json: bool) -> Result<()> {
    let body = fetch(path).await?;
    if json {
        println!("{}", body.trim_end());
    } else {
        let snapshot: Http3Snapshot = serde_json::from_str(&body)?;
        print!("{}", render(&snapshot, None));
    }
    Ok(())
}

#[cfg(unix)]
async fn fetch(path: &Path) -> Result<String> {
    use anyhow::Context;
    use tokio::{io::AsyncReadExt, net::UnixStream};

    let mut stream = UnixStream::connect(path).await.with_context(|| {
        format!(
            "cannot reach the proxy on {}; is it running with --control?",
            path.display()
        )
    })?;
    let mut body = String::new();
    stream.read_to_string(&mut body).await?;
    Ok(body)
}

#[cfg(not(unix))]
async fn fetch(_path: &Path) -> Result<String> {
    Err(anyhow::anyhow!(
        "netstat is only supported on Linux and other Unix systems"
    ))
}

/// `rates` is `None` for a single snapshot, which has no rate to show. In watch mode it is
/// `Some`, holding the previous snapshot and the time since it (`None` on the first frame).
fn render(snapshot: &Http3Snapshot, rates: Option<Option<(&Http3Snapshot, Duration)>>) -> String {
    let width = terminal_size::terminal_size().map(|(width, _)| usize::from(width.0));
    render_for_width(snapshot, rates, width)
}

/// `width` is the terminal's width in columns, `None` when output is not a terminal.
fn render_for_width(
    snapshot: &Http3Snapshot,
    rates: Option<Option<(&Http3Snapshot, Duration)>>,
    width: Option<usize>,
) -> String {
    let mut out = String::new();
    match snapshot.tcp_fallback_secs_left {
        Some(secs) => out.push_str(&format!(
            "HTTP/3 is paused: tunnels use TCP for another {secs}s\n"
        )),
        None => out.push_str("HTTP/3 is active\n"),
    }
    if snapshot.connections.is_empty() {
        out.push_str("No HTTP/3 connections\n");
        return out;
    }

    let mut header = vec![
        "Local Address",
        "Foreign Address",
        "State",
        "RTT",
        "Tunnels",
        "Lost",
        "Tx",
        "Rx",
    ];
    if rates.is_some() {
        header.extend(["Tx/s", "Rx/s"]);
    }
    header.push("Host");

    let mut rows = Vec::new();
    for conn in &snapshot.connections {
        let mut row = vec![
            conn.local_addr.clone().unwrap_or_else(|| "-".to_owned()),
            conn.remote_addr.clone(),
            conn.state.to_uppercase(),
            format!("{}ms", conn.rtt_ms),
            conn.active_tunnels.to_string(),
            conn.lost_packets.to_string(),
            bytes(conn.udp_tx_bytes),
            bytes(conn.udp_rx_bytes),
        ];
        if let Some(basis) = rates {
            // The same connection in the previous snapshot: same sockets, counters not reset.
            let earlier = basis.and_then(|(previous, elapsed)| {
                previous
                    .connections
                    .iter()
                    .find(|old| {
                        old.local_addr == conn.local_addr && old.remote_addr == conn.remote_addr
                    })
                    .map(|old| (old, elapsed))
            });
            row.push(rate(earlier, |old| (old.udp_tx_bytes, conn.udp_tx_bytes)));
            row.push(rate(earlier, |old| (old.udp_rx_bytes, conn.udp_rx_bytes)));
        }
        row.push(format!("{}:{}", conn.host, conn.port));
        rows.push(row);
    }

    // A table that does not fit the terminal would wrap into an unreadable mess, so a narrow
    // terminal gets a few lines per connection instead.
    let natural_width: usize = (0..header.len())
        .map(|column| {
            let cells = rows.iter().map(|row| row[column].chars().count());
            // Each column has one space of padding on both sides.
            cells
                .chain([header[column].chars().count()])
                .max()
                .unwrap_or(0)
                + 2
        })
        .sum();
    if width.is_some_and(|width| natural_width > width) {
        out.push_str(&compact(&header, &rows));
    } else {
        out.push_str(&table(&header, &rows));
    }
    out
}

fn table(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut table = Table::new();
    table.load_style(NOTHING);
    table.set_header(header.iter().copied());
    // RTT, Tunnels, Lost, Tx, Rx and the rates: everything between State and Host.
    for column in 3..header.len() - 1 {
        if let Some(column) = table.column_mut(column) {
            column.set_cell_alignment(CellAlignment::Right);
        }
    }
    for row in rows {
        table.add_row(row.iter().map(String::as_str));
    }
    format!("{table}\n")
}

fn compact(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for row in rows {
        let value = |name: &str| {
            header
                .iter()
                .position(|column| *column == name)
                .map_or("-", |index| row[index].as_str())
        };
        out.push_str(&format!("\n{}  {}\n", value("Host"), value("State")));
        out.push_str(&format!("  Local    {}\n", value("Local Address")));
        out.push_str(&format!("  Foreign  {}\n", value("Foreign Address")));
        out.push_str(&format!(
            "  RTT {}  Tunnels {}  Lost {}\n",
            value("RTT"),
            value("Tunnels"),
            value("Lost")
        ));
        let mut traffic = format!("  Tx {}  Rx {}", value("Tx"), value("Rx"));
        if header.contains(&"Tx/s") {
            traffic.push_str(&format!("  Tx/s {}  Rx/s {}", value("Tx/s"), value("Rx/s")));
        }
        out.push_str(&traffic);
        out.push('\n');
    }
    out
}

/// Bytes per second between two counter readings, or `-` without an earlier reading of the same
/// connection (the first frame, a new connection, or counters that went backwards).
fn rate(
    earlier: Option<(&Http3ConnInfo, Duration)>,
    counters: impl Fn(&Http3ConnInfo) -> (u64, u64),
) -> String {
    let Some((old, elapsed)) = earlier else {
        return "-".to_owned();
    };
    let (before, now) = counters(old);
    if now < before || elapsed.is_zero() {
        return "-".to_owned();
    }
    format!(
        "{}/s",
        bytes((((now - before) as f64) / elapsed.as_secs_f64()) as u64)
    )
}

fn bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_byte_counts() {
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KiB");
        assert_eq!(bytes(5 * 1024 * 1024), "5.0 MiB");
    }

    #[test]
    fn renders_connections_and_the_fallback_state() {
        let snapshot = Http3Snapshot {
            connections: vec![Http3ConnInfo {
                host: "gw.example.com".into(),
                port: 443,
                local_addr: Some("0.0.0.0:51234".into()),
                remote_addr: "1.2.3.4:443".into(),
                state: "established".into(),
                rtt_ms: 23,
                cwnd: 12000,
                lost_packets: 1,
                udp_tx_bytes: 2048,
                udp_rx_bytes: 100,
                active_tunnels: 5,
            }],
            tcp_fallback_secs_left: None,
        };
        let text = render(&snapshot, None);
        assert!(!text.contains("Tx/s"));
        assert!(text.contains("HTTP/3 is active"));
        assert!(text.contains("ESTABLISHED"));
        assert!(text.contains("1.2.3.4:443"));
        assert!(text.contains("gw.example.com:443"));

        let paused = render(
            &Http3Snapshot {
                connections: vec![],
                tcp_fallback_secs_left: Some(42),
            },
            None,
        );
        assert!(paused.contains("another 42s"));
        assert!(paused.contains("No HTTP/3 connections"));
    }

    #[test]
    fn shows_rates_between_two_snapshots() {
        let conn = |tx, rx| Http3ConnInfo {
            host: "gw.example.com".into(),
            port: 443,
            local_addr: Some("0.0.0.0:51234".into()),
            remote_addr: "1.2.3.4:443".into(),
            state: "established".into(),
            rtt_ms: 23,
            cwnd: 12000,
            lost_packets: 0,
            udp_tx_bytes: tx,
            udp_rx_bytes: rx,
            active_tunnels: 1,
        };
        let snapshot = |tx, rx| Http3Snapshot {
            connections: vec![conn(tx, rx)],
            tcp_fallback_secs_left: None,
        };
        let before = snapshot(1000, 5000);
        let now = snapshot(3048, 5000);

        let first = render(&now, Some(None));
        assert!(first.contains("Tx/s"));
        assert!(!first.contains("B/s"), "{first}");

        let text = render(&now, Some(Some((&before, Duration::from_secs(2)))));
        assert!(text.contains("1.0 KiB/s"), "{text}");
        assert!(text.contains("0 B/s"), "{text}");

        // Counters that went backwards (a new connection on the same sockets) show no rate.
        let reset = render(&before, Some(Some((&now, Duration::from_secs(2)))));
        assert!(!reset.contains("KiB/s"), "{reset}");
    }

    #[test]
    fn a_narrow_terminal_gets_a_few_lines_per_connection() {
        let snapshot = Http3Snapshot {
            connections: vec![Http3ConnInfo {
                host: "gw.example.com".into(),
                port: 443,
                local_addr: Some("[::]:38324".into()),
                remote_addr: "[2607:8700:5500:2047:2faf:26d:f987:ced7]:443".into(),
                state: "established".into(),
                rtt_ms: 23,
                cwnd: 12000,
                lost_packets: 1,
                udp_tx_bytes: 2048,
                udp_rx_bytes: 100,
                active_tunnels: 5,
            }],
            tcp_fallback_secs_left: None,
        };

        let wide = render_for_width(&snapshot, None, Some(200));
        assert!(wide.contains("Foreign Address"), "{wide}");
        let piped = render_for_width(&snapshot, None, None);
        assert!(piped.contains("Foreign Address"), "{piped}");

        let narrow = render_for_width(&snapshot, Some(None), Some(80));
        assert!(!narrow.contains("Foreign Address"), "{narrow}");
        assert!(narrow.contains("  Foreign  [2607:8700:5500:2047:2faf:26d:f987:ced7]:443"));
        assert!(narrow.contains("gw.example.com:443  ESTABLISHED"));
        assert!(narrow.contains("RTT 23ms  Tunnels 5  Lost 1"));
        assert!(
            narrow.contains("Tx 2.0 KiB  Rx 100 B  Tx/s -  Rx/s -"),
            "{narrow}"
        );
    }
}
