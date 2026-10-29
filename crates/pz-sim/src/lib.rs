//! Fixed-timestep game simulation (deterministic; independent of rendering).
use pz_world::{flag, World};

pub const TICK_HZ: f32 = 30.0;
pub const TICK_DT: f32 = 1.0 / TICK_HZ;
const WALK: f32 = 3.0; // tiles / second
const RUN: f32 = 5.5;

#[derive(Clone, Copy, Default)]
pub struct Input {
    /// Desired move direction in world tile axes (not normalised).
    pub dir: [f32; 2],
    pub run: bool,
}

#[derive(Clone, Copy)]
pub struct Player {
    /// Continuous tile coordinates; square (x, y) is centred on integer coordinates.
    pub pos: [f32; 2],
    pub z: usize,
    /// Continuous height in floors (z plus progress along a staircase).
    pub height: f32,
}

pub fn square_of(p: [f32; 2]) -> (i32, i32) { ((p[0] + 0.5).floor() as i32, (p[1] + 0.5).floor() as i32) }

impl Player {
    pub fn new(x: f32, y: f32) -> Self { Self { pos: [x, y], z: 0, height: 0.0 } }

    /// Advance one tick. Axes are resolved separately so the player slides along walls.
    pub fn tick(&mut self, input: Input, world: &mut World) {
        let before = square_of(self.pos);
        self.move_xy(input, world);
        self.update_level(before, world);
    }

    fn move_xy(&mut self, input: Input, world: &mut World) {
        let len = (input.dir[0] * input.dir[0] + input.dir[1] * input.dir[1]).sqrt();
        if len < 1e-4 { return; }
        let step = if input.run { RUN } else { WALK } * TICK_DT;
        for axis in 0..2 {
            let d = input.dir[axis] / len * step;
            if d == 0.0 { continue; }
            let mut np = self.pos;
            np[axis] += d;
            let (from, to) = (square_of(self.pos), square_of(np));
            let exits_stairs = from != to && {
                // Leaving a staircase's top step through its far edge climbs a floor, walls below notwithstanding.
                let f = world.flags(from.0, from.1, self.z);
                (f & flag::STAIR_TN != 0 && to == (from.0, from.1 - 1)) || (f & flag::STAIR_TW != 0 && to == (from.0 - 1, from.1))
            };
            if from == to || exits_stairs || world.can_step(from, to, self.z) { self.pos = np; }
        }
    }

    /// Stairs: leaving a staircase's top step climbs a floor; walking into the hole above one descends.
    fn update_level(&mut self, before: (i32, i32), world: &mut World) {
        let now = square_of(self.pos);
        let z = self.z;
        if now != before {
            let f = world.flags(before.0, before.1, z);
            let up_n = f & flag::STAIR_TN != 0 && now == (before.0, before.1 - 1);
            let up_w = f & flag::STAIR_TW != 0 && now == (before.0 - 1, before.1);
            if up_n || up_w { self.z += 1; }
        }
        let z = self.z;
        if z > 0 && !world.has_floor(now.0, now.1, z)
            && world.flags(now.0, now.1, z - 1) & (flag::STAIR_TN | flag::STAIR_TW) != 0 {
            self.z -= 1;
        }
        self.height = self.z as f32 + self.stair_progress(world);
    }

    fn stair_progress(&self, world: &mut World) -> f32 {
        let (sx, sy) = square_of(self.pos);
        let f = world.flags(sx, sy, self.z);
        // Pieces climb toward smaller coordinates; each covers a third of a floor.
        let piece = |b: u16, m: u16| if f & b != 0 { Some(0.0) } else if f & m != 0 { Some(1.0) } else { None };
        let n = piece(flag::STAIR_BN, flag::STAIR_MN).or(if f & flag::STAIR_TN != 0 { Some(2.0) } else { None });
        if let Some(i) = n { return (i + (sy as f32 + 0.5 - self.pos[1]).clamp(0.0, 1.0)) / 3.0; }
        let w = piece(flag::STAIR_BW, flag::STAIR_MW).or(if f & flag::STAIR_TW != 0 { Some(2.0) } else { None });
        if let Some(i) = w { return (i + (sx as f32 + 0.5 - self.pos[0]).clamp(0.0, 1.0)) / 3.0; }
        0.0
    }
}
