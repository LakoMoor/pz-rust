//! Transport-agnostic multiplayer core: a compact binary protocol plus an authoritative server
//! that advances the same fixed-tick simulation as the single-player client.
use pz_sim::firearm::{Firearm, Gun};
use pz_sim::{GameClock, Input, Player, Weapon, Zombie};
use pz_world::World;
use std::collections::BTreeMap;

pub mod udp;

pub type ClientId = u32;

#[derive(Debug, Clone, PartialEq)]
pub enum ClientMsg {
    Input { seq: u32, dir: [f32; 2], run: bool },
    /// Swing the equipped weapon in the facing direction.
    Attack,
    /// Open/close the nearest door.
    Interact,
    /// Fire the equipped gun in the facing direction.
    Shoot,
    /// Top up the magazine from the rounds in the player's pockets.
    Reload,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState { pub id: ClientId, pub pos: [f32; 2], pub z: u8, pub health: f32,
    /// Rounds left in the player's magazine (0 without a gun).
    pub ammo: u16 }

#[derive(Debug, Clone, PartialEq)]
pub struct ZombieState { pub pos: [f32; 2], pub z: u8, pub health: f32 }

#[derive(Debug, Clone, PartialEq)]
pub enum ServerMsg { Snapshot { tick: u32, you: ClientId, players: Vec<PlayerState>, zombies: Vec<ZombieState>, doors: Vec<(i32, i32, u8)> } }

#[derive(Debug, PartialEq)]
pub enum DecodeError { Truncated, BadTag(u8) }

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) { self.0.push(v); }
    fn u16(&mut self, v: u16) { self.0.extend_from_slice(&v.to_le_bytes()); }
    fn u32(&mut self, v: u32) { self.0.extend_from_slice(&v.to_le_bytes()); }
    fn f32(&mut self, v: f32) { self.0.extend_from_slice(&v.to_le_bytes()); }
}

struct R<'a>(&'a [u8]);
impl R<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        if self.0.len() < N { return Err(DecodeError::Truncated); }
        let (a, b) = self.0.split_at(N);
        self.0 = b;
        Ok(a.try_into().unwrap())
    }
    fn u8(&mut self) -> Result<u8, DecodeError> { Ok(self.take::<1>()?[0]) }
    fn u16(&mut self) -> Result<u16, DecodeError> { Ok(u16::from_le_bytes(self.take()?)) }
    fn u32(&mut self) -> Result<u32, DecodeError> { Ok(u32::from_le_bytes(self.take()?)) }
    fn f32(&mut self) -> Result<f32, DecodeError> { Ok(f32::from_le_bytes(self.take()?)) }
}

impl ClientMsg {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W(vec![]);
        match self {
            ClientMsg::Input { seq, dir, run } => { w.u8(1); w.u32(*seq); w.f32(dir[0]); w.f32(dir[1]); w.u8(*run as u8); }
            ClientMsg::Attack => w.u8(2),
            ClientMsg::Interact => w.u8(3),
            ClientMsg::Shoot => w.u8(4),
            ClientMsg::Reload => w.u8(5),
        }
        w.0
    }
    pub fn decode(b: &[u8]) -> Result<Self, DecodeError> {
        let mut r = R(b);
        match r.u8()? {
            1 => Ok(ClientMsg::Input { seq: r.u32()?, dir: [r.f32()?, r.f32()?], run: r.u8()? != 0 }),
            2 => Ok(ClientMsg::Attack),
            3 => Ok(ClientMsg::Interact),
            4 => Ok(ClientMsg::Shoot),
            5 => Ok(ClientMsg::Reload),
            t => Err(DecodeError::BadTag(t)),
        }
    }
}

impl ServerMsg {
    pub fn encode(&self) -> Vec<u8> {
        let ServerMsg::Snapshot { tick, you, players, zombies, doors } = self;
        let mut w = W(vec![]);
        w.u8(2); w.u32(*tick); w.u32(*you); w.u32(players.len() as u32);
        for p in players { w.u32(p.id); w.f32(p.pos[0]); w.f32(p.pos[1]); w.u8(p.z); w.f32(p.health); w.u16(p.ammo); }
        w.u32(zombies.len() as u32);
        for z in zombies { w.f32(z.pos[0]); w.f32(z.pos[1]); w.u8(z.z); w.f32(z.health); }
        w.u32(doors.len() as u32);
        for (x, y, z) in doors { w.u32(*x as u32); w.u32(*y as u32); w.u8(*z); }
        w.0
    }
    pub fn decode(b: &[u8]) -> Result<Self, DecodeError> {
        let mut r = R(b);
        match r.u8()? {
            2 => {
                let tick = r.u32()?;
                let you = r.u32()?;
                let np = r.u32()?;
                let mut players = Vec::new();
                for _ in 0..np {
                    players.push(PlayerState { id: r.u32()?, pos: [r.f32()?, r.f32()?], z: r.u8()?, health: r.f32()?, ammo: r.u16()? });
                }
                let nz = r.u32()?;
                let mut zombies = Vec::new();
                for _ in 0..nz {
                    zombies.push(ZombieState { pos: [r.f32()?, r.f32()?], z: r.u8()?, health: r.f32()? });
                }
                let nd = r.u32()?;
                let mut doors = Vec::new();
                for _ in 0..nd { doors.push((r.u32()? as i32, r.u32()? as i32, r.u8()?)); }
                Ok(ServerMsg::Snapshot { tick, you, players, zombies, doors })
            }
            t => Err(DecodeError::BadTag(t)),
        }
    }
}

/// Authoritative server state. Feed it decoded client messages; call `tick` at 30 Hz.
pub struct Server {
    world: World,
    tick: u32,
    players: BTreeMap<ClientId, Player>,
    inputs: BTreeMap<ClientId, Input>,
    zombies: Vec<Zombie>,
    weapon: Weapon,
    /// Firearm template: every joining player gets their own loaded copy.
    gun: Option<Gun>,
    guns: BTreeMap<ClientId, Gun>,
    /// Item scripts, the gun's definition and the spare rounds each joining player starts with.
    kit: Option<(pz_items::ItemDb, pz_items::ItemDef, u32)>,
    packs: BTreeMap<ClientId, pz_items::Inventory>,
    /// Game time; sets how far zombies see.
    pub clock: GameClock,
    /// Calendar and climate driving `clock` (dawn, dusk, daily fog); the game starts on 1993-07-09, like the original.
    pub climate: pz_sim::climate::Climate,
}

impl Server {
    pub fn new(world: World, zombies: Vec<Zombie>) -> Self {
        let mut clock = GameClock::default();
        let climate = new_climate(&mut clock);
        Self { world, tick: 0, players: BTreeMap::new(), inputs: BTreeMap::new(), zombies, weapon: Weapon::FISTS, clock, climate, gun: None, guns: BTreeMap::new(), kit: None, packs: BTreeMap::new() }
    }

    pub fn set_weapon(&mut self, w: Weapon) { self.weapon = w; }
    /// Give joining players this (loaded) firearm; `Shoot` does nothing without one.
    pub fn set_gun(&mut self, g: Gun) { self.gun = Some(g); }
    /// Enable reloading: `def` is the wielded firearm's script and each player joins carrying `spare_rounds` of its ammo.
    pub fn set_ammo_kit(&mut self, db: pz_items::ItemDb, def: pz_items::ItemDef, spare_rounds: u32) { self.kit = Some((db, def, spare_rounds)); }
    pub fn join(&mut self, id: ClientId, x: f32, y: f32) { self.players.insert(id, Player::new(x, y));
        if let Some(g) = self.gun { self.guns.insert(id, g); }
        if let Some((db, def, spare)) = &self.kit {
            let mut inv = pz_items::Inventory::player();
            if let Some(round) = def.ammo_type.as_deref().and_then(|a| db.ammo_item(a)) { let _ = inv.add(db, round, *spare); }
            self.packs.insert(id, inv);
        }
    }
    pub fn leave(&mut self, id: ClientId) { self.players.remove(&id); self.inputs.remove(&id); self.guns.remove(&id); self.packs.remove(&id); }

    pub fn handle(&mut self, id: ClientId, msg: ClientMsg) {
        if !self.players.contains_key(&id) { return; }
        match msg {
            ClientMsg::Input { dir, run, .. } => {
                // Never trust the client: clamp the direction so speed cannot be inflated.
                let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
                let dir = if len > 1.0 { [dir[0] / len, dir[1] / len] } else { dir };
                self.inputs.insert(id, Input { dir, run });
            }
            ClientMsg::Attack => {
                let w = self.weapon;
                if let Some(p) = self.players.get_mut(&id) { p.attack(&mut self.zombies, &w); }
            }
            ClientMsg::Shoot => {
                if let (Some(p), Some(g)) = (self.players.get_mut(&id), self.guns.get_mut(&id)) { p.shoot(&mut self.zombies, g); }
            }
            ClientMsg::Reload => {
                if let (Some((db, def, _)), Some(g), Some(inv)) = (&self.kit, self.guns.get_mut(&id), self.packs.get_mut(&id)) {
                    reload_from_inventory(def, g, inv, db);
                }
            }
            ClientMsg::Interact => {
                if let Some(p) = self.players.get(&id) { p.interact(&mut self.world); }
            }
        }
    }

    pub fn tick(&mut self) -> ServerMsg {
        for (id, p) in self.players.iter_mut() {
            p.tick(self.inputs.get(id).copied().unwrap_or_default(), &mut self.world);
        }
        self.clock.tick();
        self.climate.advance(&mut self.clock);
        let weather = self.clock.weather_now(self.climate.rain());
        // Each zombie chases the nearest living player on its floor.
        for zb in &mut self.zombies {
            // Prefer the nearest living player; with none on this floor, the nearest body to feed on.
            let dist2 = |p: &Player| (p.pos[0] - zb.pos[0]).powi(2) + (p.pos[1] - zb.pos[1]).powi(2);
            let key = self.players.iter().filter(|(_, p)| p.z == zb.z)
                .min_by(|(_, a), (_, b)| ((a.health <= 0.0), dist2(a)).partial_cmp(&((b.health <= 0.0), dist2(b))).unwrap())
                .map(|(id, _)| *id);
            let nearest = key.and_then(|id| self.players.get_mut(&id));
            if let Some(p) = nearest { zb.tick_in(p, &mut self.world, &weather); }
        }
        let sounds: Vec<_> = self.players.values_mut().flat_map(|p| std::mem::take(&mut p.sounds)).collect();
        if !sounds.is_empty() { for zb in &mut self.zombies { zb.hear(&sounds); } }
        self.tick += 1;
        ServerMsg::Snapshot {
            tick: self.tick,
            you: 0, // filled in per recipient by the transport
            players: self.players.iter().map(|(id, p)| PlayerState { id: *id, pos: p.pos, z: p.z as u8, health: p.health, ammo: self.guns.get(id).map_or(0, |g| g.ammo as u16) }).collect(),
            zombies: self.zombies.iter().map(|z| ZombieState { pos: z.pos, z: z.z as u8, health: z.health }).collect(),
            doors: self.world.open_doors().into_iter().map(|(x, y, z)| (x, y, z as u8)).collect(),
        }
    }
}

/// Build a loaded [`Gun`] from an item script; `None` unless the item is a firearm.
pub fn gun_from_item(d: &pz_items::ItemDef) -> Option<Gun> {
    d.is_firearm().then(|| Gun {
        firearm: Firearm { hit_chance: d.hit_chance, min_sight_range: d.min_sight_range, max_sight_range: d.max_sight_range },
        min_damage: d.min_damage, max_damage: d.max_damage, max_range: d.max_range,
        recoil_delay: d.recoil_delay as u32, ammo: d.max_ammo as u32,
        sound_radius: d.sound_radius, sound_volume: d.sound_volume,
    })
}

/// Reload `gun` (the firearm described by `def`) from the rounds in `inv`: moves up to a full magazine's worth.
/// Returns the number of rounds loaded; 0 when the item is no firearm, the magazine is full, or no rounds are carried.
pub fn reload_from_inventory(def: &pz_items::ItemDef, gun: &mut Gun, inv: &mut pz_items::Inventory, db: &pz_items::ItemDb) -> u32 {
    let Some(round) = def.ammo_type.as_deref().and_then(|a| db.ammo_item(a)) else { return 0 };
    let taken = gun.load(def.max_ammo as u32, inv.count(round));
    if taken > 0 { inv.remove(db, round, taken); }
    taken
}

/// Default climate: Muldraugh-like seasons, fixed noise offsets, the original's start date (9 July 1993).
pub fn new_climate(clock: &mut GameClock) -> pz_sim::climate::Climate {
    use pz_sim::climate::{Climate, Offsets, Seasons};
    Climate::new(Seasons::default(), Offsets { a: 4000.0, b: 12000.0, c: -4000.0, d: -12000.0 }, 1993, 6, 9, clock)
}
