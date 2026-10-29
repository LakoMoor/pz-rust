use pz_sim::*;
use pz_world::World;

fn dead_player_next_to(z: &mut Zombie) -> Player {
    let mut p = Player::new(z.pos[0] + 0.5, z.pos[1]);
    p.health = 0.0;
    p
}

#[test]
fn zombie_feeds_on_a_dead_player_then_loses_interest() {
    let mut w = World::new(std::path::PathBuf::from("."));
    let mut z = Zombie::new(0.0, 0.0, 0);
    let mut p = dead_player_next_to(&mut z);
    z.tick(&mut p, &mut w);
    assert!(z.is_eating());
    assert_eq!(p.health, 0.0, "a body cannot be hurt further");
    // Feeding lasts between 1800 and 3600 ticks, then the zombie is fed and stops.
    let mut ticks = 1;
    while z.is_eating() { z.tick(&mut p, &mut w); ticks += 1; assert!(ticks <= 3600); }
    assert!(ticks >= 1800, "ticks={ticks}");
    for _ in 0..30 { z.tick(&mut p, &mut w); }
    assert!(!z.is_eating(), "does not start over on the same body");
}

#[test]
fn a_living_player_interrupts_the_meal() {
    let mut w = World::new(std::path::PathBuf::from("."));
    let mut z = Zombie::new(0.0, 0.0, 0);
    let mut body = dead_player_next_to(&mut z);
    for _ in 0..10 { z.tick(&mut body, &mut w); }
    assert!(z.is_eating());
    let mut alive = Player::new(8.0, 0.0);
    z.tick(&mut alive, &mut w);
    assert!(!z.is_eating());
}

#[test]
fn eating_halves_vision() {
    let w = Weather::default();
    assert_eq!(w.vision_radius_for(Sight::Normal, 1.0, true), 10.0);
}
