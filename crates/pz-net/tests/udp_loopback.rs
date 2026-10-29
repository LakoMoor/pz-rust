use pz_net::{udp::*, *};
use pz_world::World;
use std::{thread::sleep, time::Duration};

/// Two clients talk to a server over real loopback UDP sockets.
#[test]
fn two_clients_over_loopback_udp() {
    let mut srv = UdpServer::bind("127.0.0.1:0", Server::new(World::new("nonexistent"), vec![]), [0.0, 0.0]).unwrap();
    let addr = srv.local_addr().unwrap();
    let mut a = UdpClient::connect(addr).unwrap();
    let mut b = UdpClient::connect(addr).unwrap();

    let (mut snap_a, mut snap_b) = (None, None);
    for _ in 0..90 {
        a.send_input([1.0, 0.0], false).unwrap();
        b.send_input([0.0, 1.0], false).unwrap();
        sleep(Duration::from_millis(2));
        srv.poll().unwrap();
        srv.step().unwrap();
        sleep(Duration::from_millis(2));
        snap_a = a.latest_snapshot().unwrap().or(snap_a);
        snap_b = b.latest_snapshot().unwrap().or(snap_b);
    }
    assert_eq!(srv.client_count(), 2);
    let ServerMsg::Snapshot { players, tick, .. } = snap_a.expect("client a got snapshots");
    assert!(tick > 30);
    assert_eq!(players.len(), 2);
    // Player 1 walked east, player 2 south, for roughly tick/30 seconds at 3 tiles/s.
    let p1 = players.iter().find(|p| p.id == 1).unwrap();
    let p2 = players.iter().find(|p| p.id == 2).unwrap();
    assert!(p1.pos[0] > 3.0 && p1.pos[1].abs() < 0.01, "p1 {:?}", p1.pos);
    assert!(p2.pos[1] > 3.0 && p2.pos[0].abs() < 0.01, "p2 {:?}", p2.pos);
    assert!(snap_b.is_some());
}

#[test]
fn garbage_datagrams_are_ignored() {
    let mut srv = UdpServer::bind("127.0.0.1:0", Server::new(World::new("nonexistent"), vec![]), [0.0, 0.0]).unwrap();
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.send_to(&[0xff, 1, 2, 3], srv.local_addr().unwrap()).unwrap();
    sleep(Duration::from_millis(20));
    srv.poll().unwrap();
    assert_eq!(srv.client_count(), 0, "garbage must not create a player");
}
