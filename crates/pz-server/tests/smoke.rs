use pz_net::{udp::UdpClient, ServerMsg};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Launch the real server binary (no game files needed: unknown map just means an empty world),
/// connect a client over UDP and check that snapshots come back at ~30 Hz.
#[test]
fn server_binary_serves_snapshots() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pz-server"))
        .args(["--bind", "127.0.0.1:0", "--pz", "nonexistent", "--ticks", "150"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
    let addr = line.trim().strip_prefix("listening ").expect("listening line").to_string();

    let mut c = UdpClient::connect(addr).unwrap();
    let mut last = None;
    for _ in 0..60 {
        c.send_input([0.0, -1.0], false).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        if let Some(m) = c.latest_snapshot().unwrap() { last = Some(m); }
    }
    let ServerMsg::Snapshot { tick, you, players, zombies, .. } = last.expect("snapshots received");
    assert!(tick >= 20, "tick {tick}");
    assert_eq!(you, 1);
    assert_eq!(players.len(), 1);
    assert_eq!(zombies.len(), 8);
    assert!(players[0].pos[1] < 1660.0, "player moved north: {:?}", players[0].pos);
    child.kill().ok();
}
