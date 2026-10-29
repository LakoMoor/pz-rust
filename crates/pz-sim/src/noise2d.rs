//! Layered Perlin-style noise, ported from `zombie.erosion.utils.Noise2D` (drives the seasons and day temperatures).

const PERM: [i32; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
    140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
    247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
    57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
    74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
    60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
    65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
    200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
    52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
    207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
    119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
    129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
    218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
    81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
    184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
    222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];

struct Layer { freq: f32, amp: f32, p: [i32; 512] }

#[derive(Default)]
pub struct Noise2D { layers: Vec<Layer> }

fn lerp(t: f32, a: f32, b: f32) -> f32 { a + t * (b - a) }
fn fade(t: f32) -> f32 { t * t * t * (t * (t * 6.0 - 15.0) + 10.0) }

/// `(int)Math.floor(v - Math.floor(v / m) * m)`, evaluated in double like the Java code.
fn wrap(v: f32, m: f32) -> usize {
    let v64 = v as f64;
    (v64 - ((v / m).floor() as f64) * m as f64).floor() as i32 as usize
}

impl Noise2D {
    pub fn new() -> Self { Self::default() }

    pub fn add_layer(&mut self, seed: i32, freq: f32, amp: f32) {
        let seed = seed.rem_euclid(256);
        let mut p = [0i32; 512];
        for (i, v) in PERM.iter().enumerate() {
            let ii = (seed + i as i32).rem_euclid(256) as usize;
            p[ii] = *v;
            p[256 + ii] = *v;
        }
        self.layers.push(Layer { freq, amp, p });
    }

    pub fn reset(&mut self) { self.layers.clear(); }

    fn noise(x: f32, y: f32, lp: &[i32; 512]) -> f32 {
        let (xi, yi) = (wrap(x, 255.0), wrap(y, 255.0));
        let (fx, fy) = (fade(x - x.floor()), fade(y - y.floor()));
        let (aa, ab) = (lp[xi] as usize + yi, lp[xi] as usize + yi + 1);
        let (ba, bb) = (lp[xi + 1] as usize + yi, lp[xi + 1] as usize + yi + 1);
        let at = |i: usize| PERM[lp[i] as usize & 0xFF] as f32;
        lerp(fy, lerp(fx, at(aa), at(ba)), lerp(fx, at(ab), at(bb)))
    }

    /// Weighted sum of the layers, normalised to 0..1.
    pub fn layered_noise(&self, x: f32, y: f32) -> f32 {
        let (mut sum, mut max) = (0.0f32, 0.0f32);
        for l in &self.layers {
            max += l.amp;
            sum += Self::noise(x * l.freq, y * l.freq, &l.p) * l.amp;
        }
        sum / max / 255.0
    }
}
