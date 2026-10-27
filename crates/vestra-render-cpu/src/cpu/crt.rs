//! Fixed-point inverse coordinates keep transparent CRT borders backend independent.

pub(crate) struct Sampling {
    curvature: i32,
    jitter: i32,
    sin: i32,
    cos: i32,
    seed: u32,
}

impl Sampling {
    pub(crate) fn new(curvature: f64, jitter: f64, seed: u32, phase: f32) -> Self {
        let (sin, cos) = phase.sin_cos();
        Self {
            curvature: (curvature as f32 * 32768.0).round() as i32,
            jitter: (jitter as f32 * 256.0).round() as i32,
            sin: (sin * 32768.0).round() as i32,
            cos: (cos * 32768.0).round() as i32,
            seed,
        }
    }

    pub(crate) fn position(&self, x: u32, y: u32, width: u32, height: u32) -> [f32; 2] {
        const SCALE: i32 = 32768;
        let nx = (2 * x as i32 + 1 - width as i32) * SCALE / width as i32;
        let ny = (2 * y as i32 + 1 - height as i32) * SCALE / height as i32;
        let curvature = self.curvature;
        let warp = |n: i32, other: i32, extent: u32, pixel: u32| {
            if curvature == 0 {
                return (2 * pixel as i32 + 1) * 128;
            }
            let factor = SCALE + curvature * (other * other / SCALE) / SCALE;
            (n * factor / SCALE + SCALE) * extent as i32 / 256
        };
        let mut h = (y / 4).wrapping_mul(668265263) ^ self.seed;
        h ^= h >> 16;
        h = h.wrapping_mul(0x7feb352d);
        h ^= h >> 15;
        h = h.wrapping_mul(0x846ca68b);
        h ^= h >> 16;
        let noise = (((h & 65535) as i32 - SCALE) * self.cos
            + ((h >> 16) as i32 - SCALE) * self.sin)
            / SCALE;
        let offset = self.jitter * noise / SCALE;
        [
            (warp(nx, ny, width, x) + offset) as f32 / 256.0,
            warp(ny, nx, height, y) as f32 / 256.0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inverse_sampling_is_bounded_and_identity_has_exact_centers() {
        for extent in [2, 1920, 3840, 8192] {
            for p in [0, extent / 2, extent - 1] {
                let result = Sampling::new(0.5, 8.0, 7, 0.7).position(p, p, extent, extent);
                assert!(result.into_iter().all(f32::is_finite));
            }
        }
        assert_eq!(
            Sampling::new(0.0, 0.0, 0, 0.0).position(0, 0, 2, 2),
            [0.5, 0.5]
        );
    }
}
