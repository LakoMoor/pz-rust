mod assets;
mod render;

use assets::Assets;
use render::{Batch, Instance, Renderer, ATLAS, WHITE, WHITE_UV};
use pz_sim::{square_of, Input, Player, Weapon, Zombie, TICK_DT};
use pz_world::World;
use std::{path::PathBuf, sync::Arc, time::Instant};

const HALF_W: f32 = 64.0; // 2x tiles: 128x64 diamond
const HALF_H: f32 = 32.0;
const LEVEL_PX: f32 = 192.0; // height of one floor on screen (3 tiles)
const MAX_Z: usize = 7;

struct Scene { assets: Assets, world: World, player: Player, zombies: Vec<Zombie> }

/// Continuous tile coordinates -> screen-space (pixel) position.
fn pos_to_screen(p: [f32; 2]) -> [f32; 2] { [(p[0] - p[1]) * HALF_W, (p[0] + p[1]) * HALF_H] }

/// Camera target: the player's feet, raised with their height.
fn player_cam(p: &Player) -> [f32; 2] {
    let c = pos_to_screen(p.pos);
    [c[0], c[1] - p.height * LEVEL_PX]
}

/// Tile -> screen-space (pixel) position of its diamond centre at z = 0.
fn tile_to_screen(x: i32, y: i32) -> [f32; 2] { [(x - y) as f32 * HALF_W, (x + y) as f32 * HALF_H] }

fn screen_to_tile(p: [f32; 2]) -> [f32; 2] {
    let (a, b) = (p[0] / HALF_W, p[1] / HALF_H);
    [(a + b) / 2.0, (b - a) / 2.0]
}

impl Scene {
    /// Painter's-order sprite list for everything visible around `center`.
    fn build(&mut self, center: [f32; 2], viewport: (u32, u32), zoom: f32) -> (Vec<Instance>, Vec<Batch>) {
        let (hw, hh) = (viewport.0 as f32 / (2.0 * zoom), viewport.1 as f32 / (2.0 * zoom));
        let mut inst = Vec::new();
        let mut batches: Vec<Batch> = Vec::new();
        let player_sq = square_of(self.player.pos);
        let pz = self.player.z;
        // Indoors: hide everything above the player's floor. Outdoors: show roofs too.
        let indoors = self.world.ensure(player_sq.0, player_sq.1)
            && self.world.column(player_sq.0, player_sq.1, pz).is_some_and(|c| c.room != -1);
        let top = if indoors { pz } else { MAX_Z };
        for z in 0..=top {
            // A level sits LEVEL_PX higher on screen, so it shows tiles further "down" the diagonal.
            let zc = [center[0], center[1] + z as f32 * LEVEL_PX];
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let t = screen_to_tile([zc[0] + sx * hw, zc[1] + sy * hh]);
                x0 = x0.min(t[0]); x1 = x1.max(t[0]); y0 = y0.min(t[1]); y1 = y1.max(t[1]);
            }
            // Tall sprites (up to 256px) hang above their tile: pad toward the screen top.
            let (x0, y0, x1, y1) = (x0.floor() as i32 - 2, y0.floor() as i32 - 2, x1.ceil() as i32 + 8, y1.ceil() as i32 + 8);
            for y in (y0.div_euclid(8) * 8 - 8..=y1).step_by(8) {
                for x in (x0.div_euclid(8) * 8 - 8..=x1).step_by(8) { self.world.ensure(x, y); }
            }
            let zoff = -(z as f32) * LEVEL_PX;
            // Pass 1: ground layer of every square. Pass 2: everything above, back to front.
            for pass in 0..2 {
                for d in (x0 + y0)..=(x1 + y1) {
                    for x in x0.max(d - y1)..=x1.min(d - y0) {
                        let y = d - x;
                        let c = tile_to_screen(x, y);
                        if let Some(col) = self.world.column(x, y, z) {
                            for (i, name) in col.names.iter().enumerate() {
                                if (i == 0) != (pass == 0) { continue; }
                                if self.world.is_door_tile(name) && self.world.door_open(x, y, z) { continue; }
                                let Some(s) = self.assets.sprite(name) else { continue };
                                let (pw, ph) = (ATLAS as f32, ATLAS as f32);
                                emit(&mut inst, &mut batches, s.page, Instance {
                                    pos: [c[0] + s.ox as f32 - s.ow as f32 / 2.0, c[1] + zoff + s.oy as f32 - (s.oh as f32 - HALF_H)],
                                    size: [s.w as f32, s.h as f32],
                                    uv: [s.x as f32 / pw, s.y as f32 / ph, (s.x + s.w) as f32 / pw, (s.y + s.h) as f32 / ph],
                                    color: [1.0; 4], layer: 0.0,
                                });
                            }
                        }
                        if pass == 1 {
                            for zb in self.zombies.iter().filter(|zb| zb.z == z && square_of(zb.pos) == (x, y)) {
                                let f = pos_to_screen(zb.pos);
                                emit(&mut inst, &mut batches, WHITE, Instance {
                                    pos: [f[0] - 12.0, f[1] - 80.0 - z as f32 * LEVEL_PX], size: [24.0, 80.0], uv: WHITE_UV, color: [0.35, 0.55, 0.3, 1.0], layer: 0.0,
                                });
                            }
                        }
                        if pass == 1 && z == pz && (x, y) == player_sq {
                            // Placeholder body until character models/animations are ported.
                            let f = pos_to_screen(self.player.pos);
                            emit(&mut inst, &mut batches, WHITE, Instance {
                                pos: [f[0] - 12.0, f[1] - self.player.height * LEVEL_PX - 90.0], size: [24.0, 90.0], uv: WHITE_UV, color: [0.85, 0.2, 0.15, 1.0], layer: 0.0,
                            });
                        }
                    }
                }
            }
        }
        (inst, batches)
    }
}

/// Append an instance, extending the last batch when it uses the same atlas page.
fn emit(inst: &mut Vec<Instance>, batches: &mut Vec<Batch>, page: assets::PageKey, i: Instance) {
    inst.push(i);
    let n = inst.len() as u32;
    match batches.last_mut() {
        Some(b) if b.page == page => b.range.end = n,
        _ => batches.push(Batch { page, range: n - 1..n }),
    }
}

struct Args {
    connect: Option<String>,
    pz: PathBuf, map: String, at: [f32; 2], zoom: f32, size: (u32, u32), shot: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut a = Args {
        connect: None,
        pz: std::env::var("PZ_DIR").map(PathBuf::from).unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into()),
        map: "Muldraugh, KY".into(), at: [12700.0, 1700.0], zoom: 0.5, size: (1600, 900), shot: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().unwrap_or_else(|| panic!("{k} needs a value"));
        match k.as_str() {
            "--pz" => a.pz = v().into(),
            "--connect" => a.connect = Some(v()),
            "--map" => a.map = v(),
            "--shot" => a.shot = Some(v().into()),
            "--zoom" => a.zoom = v().parse().unwrap(),
            "--at" => { let s = v(); let (x, y) = s.split_once(',').unwrap(); a.at = [x.parse().unwrap(), y.parse().unwrap()]; }
            "--size" => { let s = v(); let (w, h) = s.split_once('x').unwrap(); a.size = (w.parse().unwrap(), h.parse().unwrap()); }
            other => panic!("unknown argument {other}"),
        }
    }
    a
}

fn request_device(instance: &wgpu::Instance, surface: Option<&wgpu::Surface>) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: surface, force_fallback_adapter: false,
    })).expect("no GPU adapter");
    log::info!("adapter: {:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).expect("device");
    (adapter, device, queue)
}

fn make_scene(a: &Args) -> Scene {
    let t = Instant::now();
    let assets = Assets::load(&a.pz.join("media")).expect("load texture packs");
    log::info!("indexed {} sprites in {:?}", assets.sprite_count(), t.elapsed());
    let mut world = World::new(a.pz.join("media").join("maps").join(&a.map));
    world.load_tile_flags(&a.pz.join("media"));
    // Spawn on the nearest walkable square (solid squares are skipped).
    let (cx, cy) = (a.at[0] as i32, a.at[1] as i32);
    let (sx, sy) = (0..40i32).flat_map(|r| (-r..=r).flat_map(move |i| [(i, -r), (i, r), (-r, i), (r, i)]))
        .map(|(dx, dy)| (cx + dx, cy + dy))
        .find(|&(x, y)| world.flags(x, y, 0) & pz_world::flag::SOLID == 0 && world.column(x, y, 0).is_some())
        .unwrap_or((cx, cy));
    // A handful of zombies ringed around the spawn point (walkable squares only).
    let zombies = [(-7, -2), (6, 5), (-4, 8), (9, -6), (2, -9)].iter()
        .map(|&(dx, dy)| (sx + dx, sy + dy))
        .filter(|&(x, y)| world.has_floor(x, y, 0) && world.flags(x, y, 0) & pz_world::flag::SOLID == 0)
        .map(|(x, y)| Zombie::new(x as f32, y as f32, 0)).collect();
    Scene { assets, world, player: Player::new(sx as f32, sy as f32), zombies }
}

/// Render one frame to a PNG without opening a window (also the cross-platform regression hook).
fn screenshot(a: &Args, path: &PathBuf) {
    let mut scene = make_scene(a);
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let (_ad, device, queue) = request_device(&instance, None);
    let mut r = Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm);
    let center = player_cam(&scene.player);
    let t = Instant::now();
    let (mut inst, batches) = scene.build(center, a.size, a.zoom);
    r.prepare(&scene.assets, &mut inst, &batches);
    log::info!("{} sprites, {} batches -> {} draw calls, built+uploaded in {:?}", inst.len(), batches.len(), r.draw_calls(), t.elapsed());

    let (w, h) = a.size;
    let tex = r.device.create_texture(&wgpu::TextureDescriptor {
        label: None, size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 }, mip_level_count: 1,
        sample_count: 1, dimension: wgpu::TextureDimension::D2, format: r.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC, view_formats: &[],
    });
    r.draw(&tex.create_view(&Default::default()), a.size, center, a.zoom, &inst);
    let row = (w * 4).div_ceil(256) * 256;
    let buf = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: None, size: (row * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false,
    });
    let mut enc = r.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    r.queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |res| res.expect("map"));
    r.device.poll(wgpu::Maintain::Wait);
    let data = buf.slice(..).get_mapped_range();
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize { rgba.extend_from_slice(&data[y * row as usize..][..(w * 4) as usize]); }
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    println!("wrote {}", path.display());
}

// ---- interactive window ----------------------------------------------------------------------

use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

struct Gfx { window: Arc<Window>, surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration, r: Renderer }

struct App {
    args: Args, scene: Scene, gfx: Option<Gfx>, center: [f32; 2], zoom: f32,
    keys: std::collections::HashSet<KeyCode>, last: Instant, acc: f32, weapon: Weapon, net: Option<pz_net::udp::UdpClient>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        let attrs = Window::default_attributes().with_title("pz-rust").with_inner_size(winit::dpi::PhysicalSize::new(self.args.size.0, self.args.size.1));
        let window = Arc::new(el.create_window(attrs).unwrap());
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();
        let (adapter, device, queue) = request_device(&instance, Some(&surface));
        let caps = surface.get_capabilities(&adapter);
        // Atlas pixels are authored in display space: use a non-sRGB surface to avoid double conversion.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format, width: size.width.max(1), height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync, desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0], view_formats: vec![],
        };
        surface.configure(&device, &config);
        self.gfx = Some(Gfx { window, surface, config, r: Renderer::new(device, queue, format) });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        let Some(g) = self.gfx.as_mut() else { return };
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) if s.width > 0 && s.height > 0 => {
                g.config.width = s.width; g.config.height = s.height;
                g.surface.configure(&g.r.device, &g.config);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(k) = event.physical_key {
                    match event.state { ElementState::Pressed => { self.keys.insert(k); } ElementState::Released => { self.keys.remove(&k); } }
                    if k == KeyCode::Escape { el.exit(); }
                    if k == KeyCode::Space && event.state == ElementState::Pressed && !event.repeat {
                        if let Some(net) = self.net.as_mut() { let _ = net.send(&pz_net::ClientMsg::Attack); } else {
                            let w = self.weapon;
                            let scn = &mut self.scene;
                            scn.player.attack(&mut scn.zombies, &w);
                        }
                    }
                    if k == KeyCode::KeyE && event.state == ElementState::Pressed && !event.repeat {
                        if let Some(net) = self.net.as_mut() { let _ = net.send(&pz_net::ClientMsg::Interact); } else {
                            self.scene.player.interact(&mut self.scene.world);
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta { MouseScrollDelta::LineDelta(_, y) => y, MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0 };
                self.zoom = (self.zoom * 1.1f32.powf(d)).clamp(0.15, 3.0);
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last).as_secs_f32().min(0.1);
                self.last = now;
                // Screen-relative controls: W/S/A/D move along the isometric screen axes.
                let mut dir = [0.0f32; 2];
                for k in &self.keys {
                    let (dx, dy) = match k {
                        KeyCode::KeyW | KeyCode::ArrowUp => (-1.0, -1.0),
                        KeyCode::KeyS | KeyCode::ArrowDown => (1.0, 1.0),
                        KeyCode::KeyA | KeyCode::ArrowLeft => (-1.0, 1.0),
                        KeyCode::KeyD | KeyCode::ArrowRight => (1.0, -1.0),
                        _ => continue,
                    };
                    dir[0] += dx; dir[1] += dy;
                }
                let input = Input { dir, run: self.keys.contains(&KeyCode::ShiftLeft) };
                self.acc += dt;
                while self.acc >= TICK_DT {
                    if let Some(net) = self.net.as_mut() {
                        let _ = net.send_input(input.dir, input.run);
                        self.acc -= TICK_DT;
                        continue;
                    }
                    self.scene.player.tick(input, &mut self.scene.world);
                    let scn = &mut self.scene;
                    for zb in &mut scn.zombies { zb.tick(&mut scn.player, &mut scn.world); }
                    self.acc -= TICK_DT;
                }
                if let Some(net) = self.net.as_mut() {
                    if let Ok(Some(pz_net::ServerMsg::Snapshot { you, players, zombies, doors, .. })) = net.latest_snapshot() {
                        self.scene.world.set_open_doors(doors.into_iter().map(|(x, y, z)| (x, y, z as usize)));
                        if let Some(me) = players.iter().find(|p| p.id == you) {
                            let pl = &mut self.scene.player;
                            // Snap on floor change or big jumps; otherwise ease toward the server position.
                            let far = (pl.pos[0] - me.pos[0]).abs() + (pl.pos[1] - me.pos[1]).abs() > 3.0;
                            if far || pl.z != me.z as usize { pl.pos = me.pos; } else {
                                pl.pos = [pl.pos[0] + (me.pos[0] - pl.pos[0]) * 0.35, pl.pos[1] + (me.pos[1] - pl.pos[1]) * 0.35];
                            }
                            pl.z = me.z as usize; pl.height = me.z as f32; pl.health = me.health;
                        }
                        self.scene.zombies = zombies.iter().map(|z| { let mut zb = Zombie::new(z.pos[0], z.pos[1], z.z as usize); zb.health = z.health; zb }).collect();
                    }
                }
                self.center = player_cam(&self.scene.player);
                g.window.set_title(&format!("pz-rust | health {:.0}{}", self.scene.player.health, if self.scene.player.health <= 0.0 { " | DEAD" } else { "" }));
                let size = (g.config.width, g.config.height);
                let (mut inst, batches) = self.scene.build(self.center, size, self.zoom);
                g.r.prepare(&self.scene.assets, &mut inst, &batches);
                match g.surface.get_current_texture() {
                    Ok(frame) => {
                        let view = frame.texture.create_view(&Default::default());
                        g.r.draw(&view, size, self.center, self.zoom, &inst);
                        g.window.pre_present_notify();
                        frame.present();
                    }
                    Err(_) => g.surface.configure(&g.r.device, &g.config),
                }
                g.window.request_redraw();
            }
            _ => {}
        }
    }
}

/// `--connect host:port` switches to client mode: the server owns the simulation, we only render it.
fn app_net(a: &Args) -> Option<pz_net::udp::UdpClient> {
    let addr = a.connect.as_ref()?;
    Some(pz_net::udp::UdpClient::connect(addr).unwrap_or_else(|e| panic!("connect {addr}: {e}")))
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn")).init();
    let args = parse_args();
    if let Some(path) = args.shot.clone() { return screenshot(&args, &path); }
    let scene = make_scene(&args);
    let center = player_cam(&scene.player);
    let zoom = args.zoom;
    // Wield a baseball bat when the item scripts are available (stats come straight from the game's data).
    let weapon = pz_items::ItemDb::load(&args.pz.join("media").join("scripts")).ok()
        .and_then(|db| db.get("Base.BaseballBat").map(|b| Weapon {
            damage: (b.min_damage + b.max_damage) / 2.0, range: b.max_range,
            cooldown_ticks: (b.swing_time * 10.0) as u32,
            crit_chance: b.crit_chance, crit_mult: b.crit_dmg_multiplier,
        }))
        .unwrap_or(Weapon::FISTS);
    log::info!("weapon: {weapon:?}");
    let net = app_net(&args);
    let mut app = App { args, scene, gfx: None, center, zoom, keys: Default::default(), last: Instant::now(), acc: 0.0, weapon, net };
    EventLoop::new().unwrap().run_app(&mut app).unwrap();
}
