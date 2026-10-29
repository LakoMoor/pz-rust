use pz_net::*;
use pz_sim::Zombie;
use pz_world::World;

fn server() -> Server { Server::new(World::new("nonexistent"), vec![]) }

#[test]
fn messages_round_trip_and_reject_garbage() {
    let c = ClientMsg::Input { seq: 7, dir: [0.5, -1.0], run: true };
    assert_eq!(ClientMsg::decode(&c.encode()), Ok(c));
    let s = ServerMsg::Snapshot {
        tick: 3,
        players: vec![PlayerState { id: 1, pos: [1.5, 2.5], z: 1, health: 90.0 }],
        zombies: vec![ZombieState { pos: [0.0, 4.0], z: 0, health: 3.0 }],
    };
    assert_eq!(ServerMsg::decode(&s.encode()), Ok(s.clone()));
    let bytes = s.encode();
    assert_eq!(ServerMsg::decode(&bytes[..bytes.len() - 1]), Err(DecodeError::Truncated));
    assert_eq!(ClientMsg::decode(&[9]), Err(DecodeError::BadTag(9)));
}

#[test]
fn authoritative_movement_is_clamped_and_deterministic() {
    let run = || {
        let mut s = server();
        s.join(1, 0.0, 0.0);
        s.join(2, 10.0, 10.0);
        // Client 1 tries to cheat with a huge direction vector.
        s.handle(1, ClientMsg::Input { seq: 1, dir: [1000.0, 0.0], run: false });
        s.handle(2, ClientMsg::Input { seq: 1, dir: [0.0, 1.0], run: false });
        let mut last = None;
        for _ in 0..30 { last = Some(s.tick()); }
        last.unwrap()
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b, "same inputs must give the same snapshot");
    let ServerMsg::Snapshot { players, .. } = a;
    assert!((players[0].pos[0] - 3.0).abs() < 0.05, "1s of walking is ~3 tiles, got {}", players[0].pos[0]);
    assert!((players[1].pos[1] - 13.0).abs() < 0.05);
}

#[test]
fn zombies_chase_the_nearest_player() {
    let mut s = Server::new(World::new("nonexistent"), vec![Zombie::new(0.0, 0.0, 0)]);
    s.join(1, 8.0, 0.0);
    s.join(2, 0.0, 3.0);
    let mut snap = None;
    for _ in 0..30 { snap = Some(s.tick()); }
    let ServerMsg::Snapshot { zombies, players, .. } = snap.unwrap();
    assert!(zombies[0].pos[1] > 0.5, "zombie heads toward player 2 (y axis): {:?}", zombies[0].pos);
    assert_eq!(players[0].health, 100.0);
}
