use pz_sim::*;
use pz_world::World;

#[test]
fn zombie_closes_in_across_open_street() {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let media = std::path::PathBuf::from(dir).join("media");
    if !media.is_dir() { return; }
    let mut w = World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    // Road in front of the bank: find a straight 8-tile stretch with nothing solid or walled.
    let mut done = false;
    'outer: for y in 1600..1700 { for x in 12600..12690 {
        let clear = (0..9).all(|i| w.has_floor(x + i, y, 0) == w.has_floor(x, y, 0)
            && w.can_step((x + i, y), (x + i + 1, y), 0));
        if clear && w.column(x, y, 0).is_some() {
            let mut player = Player::new((x + 8) as f32, y as f32);
            let mut z = Zombie::new(x as f32, y as f32, 0);
            let start = player.pos[0] - z.pos[0];
            for _ in 0..(10.0 * TICK_HZ) as u32 { z.tick(&mut player, &mut w); }
            assert!(player.pos[0] - z.pos[0] < start - 5.0, "zombie moved only {}", start - (player.pos[0] - z.pos[0]));
            done = true;
            break 'outer;
        }
    }}
    assert!(done, "no clear stretch found");
}

#[test]
fn zombie_ignores_other_floors() {
    let dir = std::env::var("PZ_DIR").unwrap_or_default();
    let _ = dir;
    let media = std::path::PathBuf::from(r"nonexistent");
    let mut w = World::new(media);
    let mut p = Player::new(5.0, 5.0);
    p.z = 1;
    let mut z = Zombie::new(0.0, 5.0, 0);
    z.tick(&mut p, &mut w);
    assert_eq!(z.pos, [0.0, 5.0]);
}

#[test]
fn zombie_bites_on_a_cooldown() {
    let mut w = World::new(std::path::PathBuf::from("nonexistent"));
    let mut p = Player::new(5.0, 5.0);
    let mut z = Zombie::new(4.5, 5.0, 0);
    z.tick(&mut p, &mut w);
    assert_eq!(p.health, 88.0);
    for _ in 0..44 { z.tick(&mut p, &mut w); }
    assert_eq!(p.health, 88.0, "no second bite during the cooldown");
    z.tick(&mut p, &mut w);
    assert_eq!(p.health, 76.0);
}
