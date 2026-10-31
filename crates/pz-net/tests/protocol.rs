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
        players: vec![PlayerState { id: 1, pos: [1.5, 2.5], z: 1, health: 90.0, ammo: 12 }],
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

#[test]
fn snapshots_report_each_players_remaining_ammo() {
    use pz_sim::firearm::{Firearm, Gun};
    let mut s = Server::new(World::new("nonexistent"), vec![]);
    s.set_gun(Gun { firearm: Firearm { hit_chance: 50.0, min_sight_range: 2.0, max_sight_range: 6.0 }, min_damage: 1.0, max_damage: 1.0, max_range: 10.0, recoil_delay: 1, ammo: 3, sound_radius: 10.0, sound_volume: 10.0 });
    s.join(1, 0.0, 0.0);
    s.join(2, 5.0, 0.0);
    s.handle(1, ClientMsg::Shoot);
    let ServerMsg::Snapshot { players, .. } = s.tick();
    let ammo = |id| players.iter().find(|p| p.id == id).unwrap().ammo;
    assert_eq!((ammo(1), ammo(2)), (2, 3));
    // The wire format carries it.
    let snap = ServerMsg::Snapshot { tick: 1, you: 1, players, zombies: vec![], doors: vec![] };
    assert_eq!(ServerMsg::decode(&snap.encode()), Ok(snap));
}

#[test]
fn reload_message_refills_from_the_players_pockets() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let scripts = std::path::PathBuf::from(dir).join("media").join("scripts");
    if !scripts.is_dir() { return; }
    assert_eq!(ClientMsg::decode(&ClientMsg::Reload.encode()), Ok(ClientMsg::Reload));
    let db = pz_items::ItemDb::load(&scripts).unwrap();
    let def = db.get("Base.Pistol").cloned().unwrap();
    let mut gun = gun_from_item(&def).unwrap();
    gun.ammo = 1;
    let mut s = Server::new(World::new("nonexistent"), vec![]);
    s.set_gun(gun);
    s.set_ammo_kit(db, def, 20);
    s.join(1, 0.0, 0.0);
    let ammo = |s: &mut Server| { let ServerMsg::Snapshot { players, .. } = s.tick(); players[0].ammo };
    assert_eq!(ammo(&mut s), 1);
    s.handle(1, ClientMsg::Reload);
    assert_eq!(ammo(&mut s), 15);
    // 20 spare - 14 used = 6 left: one more reload cannot fill it past the pockets.
    for _ in 0..15 { s.tick(); s.handle(1, ClientMsg::Shoot); for _ in 0..13 { s.tick(); } }
    assert_eq!(ammo(&mut s), 0);
    s.handle(1, ClientMsg::Reload);
    assert_eq!(ammo(&mut s), 6);
}

#[test]
fn climate_options_parse_validate_and_build_the_requested_world() {
    let mut o = ClimateOptions::default();
    assert_eq!(o.set("--pz", "x"), Ok(false));
    assert_eq!(o.set("--date", "1994-12-22"), Ok(true));
    assert_eq!(o.set("--hour", "18.5"), Ok(true));
    assert_eq!(o.set("--cycle", "night"), Ok(true));
    assert_eq!(o.set("--rain-setting", "5"), Ok(true));
    assert!(o.set("--date", "1994-13-01").is_err());
    assert!(o.set("--hour", "25").is_err());
    assert!(o.set("--cycle", "dusk").is_err());
    assert!(o.set("--rain-setting", "9").is_err());
    let (clock, climate) = o.build();
    assert_eq!((climate.year, climate.month, climate.day, climate.sandbox_rain), (1994, 11, 22, 5));
    assert_eq!(clock.hour, 18.5);
    assert_eq!(clock.night_strength(), 1.0);
    assert!((clock.dawn - 7.82).abs() < 0.05, "midwinter dawn {}", clock.dawn);
}

#[test]
fn an_infected_player_eventually_dies_on_the_server() {
    let mut s = server();
    s.mortality = 4; // 3-12 hours
    s.clock.day_seconds = 120.0; // fast days so the test is short: 1 in-game hour = 5 s
    s.join(1, 0.0, 0.0);
    s.player_mut(1).unwrap().infected = true;
    s.join(2, 20.0, 0.0);
    let health = |s: &mut Server, id| { let ServerMsg::Snapshot { players, .. } = s.tick(); players.iter().find(|p| p.id == id).unwrap().health };
    // 13 in-game hours is past the worst case.
    let ticks = (13.0 * 5.0 * 30.0) as usize;
    let mut mid = 100.0;
    for i in 0..ticks { let h = health(&mut s, 1); if i == ticks / 4 { mid = h; } }
    assert!(mid < 100.0, "sickness should have capped health by a quarter of the way: {mid}");
    assert_eq!(health(&mut s, 1), 0.0);
    assert_eq!(health(&mut s, 2), 100.0, "healthy players are unaffected");
}

#[test]
fn players_get_thirsty_over_game_time_and_runners_faster() {
    let mut s = server();
    s.clock.day_seconds = 60.0; // a game day per minute
    s.join(1, 0.0, 0.0);
    s.join(2, 20.0, 0.0);
    s.handle(2, ClientMsg::Input { seq: 1, dir: [0.0, 1.0], run: true });
    for _ in 0..(30 * 30) { s.tick(); }
    let idle = s.player_mut(1).unwrap().needs;
    let runner = s.player_mut(2).unwrap().needs;
    assert!(idle.thirst > 0.0 && idle.hunger > 0.0);
    assert!(runner.thirst > idle.thirst, "running makes you thirstier: {} vs {}", runner.thirst, idle.thirst);
}

#[test]
fn the_power_grid_fails_after_the_sandbox_shutoff_day() {
    let mut s = server();
    assert!(s.power_on());
    s.world_hours = 14.0 * 24.0;
    assert!(s.power_on(), "still on during day 14");
    s.world_hours = 14.0 * 24.0 + 1.0;
    assert!(!s.power_on());
    s.elec_shut_days = 30;
    assert!(s.power_on());
}

#[test]
fn wearing_clothing_adds_insulation_to_the_covered_parts() {
    let mut s = server();
    s.join(1, 0.0, 0.0);
    let coat = pz_items::ItemDef { blood_location: vec!["Jacket".into()], insulation: 0.8, wind_resistance: 0.5, ..Default::default() };
    assert!(s.wear(1, &coat));
    assert!(!s.wear(1, &pz_items::ItemDef::default()), "an item with no coverage is not clothing");
    assert!(!s.wear(2, &coat), "unknown player");
    let p = s.player_mut(1).unwrap();
    let (ins, _) = pz_sim::clothing::node_values(&p.worn);
    assert!(ins[6] > 1.0 && ins[16] == 0.0);
}

#[test]
fn clothes_can_be_taken_off_again() {
    let mut s = server();
    s.join(1, 0.0, 0.0);
    let hat = pz_items::ItemDef { blood_location: vec!["Head".into()], insulation: 0.9, wind_resistance: 0.9, ..Default::default() };
    assert!(s.wear(1, &hat));
    assert!(!s.take_off(1, 1) && !s.take_off(9, 0));
    assert!(s.take_off(1, 0));
    assert!(s.player_mut(1).unwrap().worn.is_empty());
}

#[test]
fn barricade_message_nails_a_plank_over_the_nearest_door() {
    assert_eq!(ClientMsg::decode(&ClientMsg::Barricade.encode()), Ok(ClientMsg::Barricade));
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let media = std::path::PathBuf::from(dir).join("media");
    if !media.is_dir() { return; }
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    let (mut dx, mut dy) = (0, 0);
    'f: for y in 1500..1800 { for x in 12500..12800 {
        if w.flags(x, y, 0) & pz_world::flag::DOOR_N != 0 { (dx, dy) = (x, y); break 'f; }
    } }
    assert!(dx != 0, "a door near Muldraugh");
    let mut s = Server::new(w, vec![]);
    s.join(1, dx as f32 + 0.5, dy as f32 + 0.5);
    for _ in 0..5 { s.handle(1, ClientMsg::Barricade); }
    let b = s.world().barricade((dx, dy, 0, true)).expect("barricaded");
    assert_eq!(b.num_planks(), 4, "the fifth plank does not fit");
}
