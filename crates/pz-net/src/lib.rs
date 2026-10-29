//! Transport-agnostic multiplayer core: a compact binary protocol plus an authoritative server
//! that advances the same fixed-tick simulation as the single-player client.
use pz_sim::{Input, Player, Weapon, Zombie};
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState { pub id: ClientId, pub pos: [f32; 2], pub z: u8, pub health: f32 }

#[derive(Debug, Clone, PartialEq)]
pub struct ZombieState { pub pos: [f32; 2], pub z: u8, pub health: f32 }

#[derive(Debug, Clone, PartialEq)]
pub enum ServerMsg { Snapshot { tick: u32, you: ClientId, players: Vec<PlayerState>, zombies: Vec<ZombieState>, doors: Vec<(i32, i32, u8)> } }

#[derive(Debug, PartialEq)]
pub enum DecodeError { Truncated, BadTag(u8) }

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) { self.0.push(v); }
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
        }
        w.0
    }
    pub fn decode(b: &[u8]) -> Result<Self, DecodeError> {
        let mut r = R(b);
        match r.u8()? {
            1 => Ok(ClientMsg::Input { seq: r.u32()?, dir: [r.f32()?, r.f32()?], run: r.u8()? != 0 }),
            2 => Ok(ClientMsg::Attack),
            3 => Ok(ClientMsg::Interact),
            t => Err(DecodeError::BadTag(t)),
        }
    }
}

impl ServerMsg {
    pub fn encode(&self) -> Vec<u8> {
        let ServerMsg::Snapshot { tick, you, players, zombies, doors } = self;
        let mut w = W(vec![]);
        w.u8(2); w.u32(*tick); w.u32(*you); w.u32(players.len() as u32);
        for p in players { w.u32(p.id); w.f32(p.pos[0]); w.f32(p.pos[1]); w.u8(p.z); w.f32(p.health); }
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
                    players.push(PlayerState { id: r.u32()?, pos: [r.f32()?, r.f32()?], z: r.u8()?, health: r.f32()? });
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
}

impl Server {
    pub fn new(world: World, zombies: Vec<Zombie>) -> Self {
        Self { world, tick: 0, players: BTreeMap::new(), inputs: BTreeMap::new(), zombies, weapon: Weapon::FISTS }
    }

    pub fn set_weapon(&mut self, w: Weapon) { self.weapon = w; }
    pub fn join(&mut self, id: ClientId, x: f32, y: f32) { self.players.insert(id, Player::new(x, y)); }
    pub fn leave(&mut self, id: ClientId) { self.players.remove(&id); self.inputs.remove(&id); }

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
            ClientMsg::Interact => {
                if let Some(p) = self.players.get(&id) { p.interact(&mut self.world); }
            }
        }
    }

    pub fn tick(&mut self) -> ServerMsg {
        for (id, p) in self.players.iter_mut() {
            p.tick(self.inputs.get(id).copied().unwrap_or_default(), &mut self.world);
        }
        // Each zombie chases the nearest living player on its floor.
        for zb in &mut self.zombies {
            let nearest = self.players.values_mut().filter(|p| p.z == zb.z && p.health > 0.0).min_by(|a, b| {
                let d = |p: &Player| (p.pos[0] - zb.pos[0]).powi(2) + (p.pos[1] - zb.pos[1]).powi(2);
                d(a).total_cmp(&d(b))
            });
            if let Some(p) = nearest { zb.tick(p, &mut self.world); }
        }
        self.tick += 1;
        ServerMsg::Snapshot {
            tick: self.tick,
            you: 0, // filled in per recipient by the transport
            players: self.players.iter().map(|(id, p)| PlayerState { id: *id, pos: p.pos, z: p.z as u8, health: p.health }).collect(),
            zombies: self.zombies.iter().map(|z| ZombieState { pos: z.pos, z: z.z as u8, health: z.health }).collect(),
            doors: self.world.open_doors().into_iter().map(|(x, y, z)| (x, y, z as u8)).collect(),
        }
    }
}
