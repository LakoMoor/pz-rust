//! Fixed-timestep game simulation (deterministic; independent of rendering).
use pz_world::World;

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
}

pub fn square_of(p: [f32; 2]) -> (i32, i32) { ((p[0] + 0.5).floor() as i32, (p[1] + 0.5).floor() as i32) }

impl Player {
    pub fn new(x: f32, y: f32) -> Self { Self { pos: [x, y], z: 0 } }

    /// Advance one tick. Axes are resolved separately so the player slides along walls.
    pub fn tick(&mut self, input: Input, world: &mut World) {
        let len = (input.dir[0] * input.dir[0] + input.dir[1] * input.dir[1]).sqrt();
        if len < 1e-4 { return; }
        let step = if input.run { RUN } else { WALK } * TICK_DT;
        for axis in 0..2 {
            let d = input.dir[axis] / len * step;
            if d == 0.0 { continue; }
            let mut np = self.pos;
            np[axis] += d;
            let (from, to) = (square_of(self.pos), square_of(np));
            if from == to || world.can_step(from, to, self.z) { self.pos = np; }
        }
    }
}
