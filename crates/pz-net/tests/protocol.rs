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
        you: 1,
        doors: vec![(5, -3, 1)],
        players: vec![PlayerState { id: 1, pos: [1.5, 2.5], z: 1, health: 90.0 }],
        zombies: vec![ZombieState { pos: [0.0, 4.0], z: 0, health: 3.0 }],
    };
    assert_eq!(ServerMsg::decode(&s.encode()), Ok(s.clone()));
    let bytes = s.encode();
    assert_eq!(ServerMsg::decode(&bytes[..bytes.len() - 1]), Err(DecodeError::Truncated));
    for m in [ClientMsg::Attack, ClientMsg::Interact] { assert_eq!(ClientMsg::decode(&m.encode()), Ok(m)); }
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

#[test]
fn attack_message_kills_zombies_on_the_server() {
    use pz_sim::Weapon;
    let mut s = Server::new(World::new("nonexistent"), vec![Zombie::new(0.8, 0.0, 0)]);
    s.set_weapon(Weapon { damage: 5.0, range: 1.2, cooldown_ticks: 10, crit_chance: 0.0, crit_mult: 1.0, push_back_mod: 1.0, small_blade: false, sound_radius: 0.0, sound_volume: 0.0 });
    s.join(1, 0.0, 0.0);
    s.handle(1, ClientMsg::Input { seq: 1, dir: [1.0, 0.0], run: false });
    s.tick(); // face +x
    s.handle(1, ClientMsg::Input { seq: 2, dir: [0.0, 0.0], run: false });
    s.handle(1, ClientMsg::Attack);
    let ServerMsg::Snapshot { zombies, .. } = s.tick();
    assert!(zombies.is_empty(), "zombie should be dead: {zombies:?}");
}

#[test]
fn shoot_message_round_trips_and_fires_the_players_own_gun() {
    use pz_sim::firearm::{Firearm, Gun};
    assert_eq!(ClientMsg::decode(&ClientMsg::Shoot.encode()), Ok(ClientMsg::Shoot));
    let gun = Gun {
        firearm: Firearm { hit_chance: 100.0, min_sight_range: 0.0, max_sight_range: 20.0 },
        min_damage: 5.0, max_damage: 5.0, max_range: 15.0, recoil_delay: 5, ammo: 1, sound_radius: 50.0, sound_volume: 50.0,
    };
    let mut s = Server::new(World::new("nonexistent"), vec![Zombie::new(4.0, 0.0, 0)]);
    s.set_gun(gun);
    s.join(1, 0.0, 0.0);
    s.handle(1, ClientMsg::Input { seq: 1, dir: [1.0, 0.0], run: false });
    s.tick(); // face +x
    s.handle(1, ClientMsg::Input { seq: 2, dir: [0.0, 0.0], run: false });
    // Players who never joined with a gun cannot shoot (no panic), and the loaded one does.
    s.handle(1, ClientMsg::Shoot);
    let ServerMsg::Snapshot { zombies, .. } = s.tick();
    assert!(zombies.is_empty() || zombies[0].health < 1.95, "the bullet should have landed: {zombies:?}");
}

#[test]
fn reloading_moves_rounds_from_the_inventory_into_the_magazine() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let scripts = std::path::PathBuf::from(dir).join("media").join("scripts");
    if !scripts.is_dir() { return; }
    let db = pz_items::ItemDb::load(&scripts).unwrap();
    let def = db.get("Base.Pistol").unwrap();
    let mut gun = gun_from_item(def).unwrap();
    gun.ammo = 3;
    let mut inv = pz_items::Inventory::new(100.0);
    inv.add(&db, "Base.Bullets9mm", 20).unwrap();
    assert_eq!(reload_from_inventory(def, &mut gun, &mut inv, &db), 12);
    assert_eq!((gun.ammo, inv.count("Base.Bullets9mm")), (15, 8));
    // Full magazine: nothing moves. Empty pockets: nothing moves.
    assert_eq!(reload_from_inventory(def, &mut gun, &mut inv, &db), 0);
    gun.ammo = 0;
    inv.remove(&db, "Base.Bullets9mm", 8);
    assert_eq!(reload_from_inventory(def, &mut gun, &mut inv, &db), 0);
    assert_eq!(gun.ammo, 0);
}
