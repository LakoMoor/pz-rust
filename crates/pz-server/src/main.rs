//! Headless dedicated server: `pz-server [--bind ADDR] [--pz DIR] [--map NAME] [--ticks N]`.
//! Prints `listening <addr>` once bound, then runs the 30 Hz simulation (forever, or N ticks).
use pz_net::{udp::UdpServer, Server};
use pz_sim::{Zombie, TICK_DT};
use pz_world::World;
use std::{path::PathBuf, time::{Duration, Instant}};

fn main() {
    let mut bind = "0.0.0.0:16261".to_string();
    let mut pz = PathBuf::from(std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into()));
    let mut map = "Muldraugh, KY".to_string();
    let mut ticks: Option<u64> = None;
    let mut args = std::env::args().skip(1);
    while let Some(k) = args.next() {
        let mut v = || args.next().unwrap_or_else(|| panic!("{k} needs a value"));
        match k.as_str() {
            "--bind" => bind = v(),
            "--pz" => pz = v().into(),
            "--map" => map = v(),
            "--ticks" => ticks = Some(v().parse().expect("--ticks N")),
            other => panic!("unknown argument {other}"),
        }
    }
    let media = pz.join("media");
    let mut world = World::new(media.join("maps").join(&map));
    world.load_tile_flags(&media);
    let spawn = [12670.0, 1660.0];
    let zombies = (0..8).map(|i| Zombie::new(spawn[0] + 6.0 + i as f32, spawn[1] + (i % 3) as f32, 0)).collect();
    let mut srv = UdpServer::bind(&bind, Server::new(world, zombies), spawn).expect("bind");
    println!("listening {}", srv.local_addr().unwrap());

    let step = Duration::from_secs_f32(TICK_DT);
    let mut next = Instant::now();
    let mut n = 0u64;
    while ticks.is_none_or(|t| n < t) {
        srv.poll().expect("poll");
        srv.step().expect("step");
        n += 1;
        next += step;
        if let Some(d) = next.checked_duration_since(Instant::now()) { std::thread::sleep(d); } else { next = Instant::now(); }
    }
}
