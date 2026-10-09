//! Shared bounded Gaussian kernels and quantization-input integer weights.

use std::cell::RefCell;
use vestra_core::effects::canonical_gaussian_radius;

#[derive(Clone)]
pub(crate) struct GaussianKernel {
    #[cfg(feature = "cpu")]
    pub(crate) weights: Vec<f64>,
    pub(crate) integer_weights: [u32; 33],
    pub(crate) radius: i32,
}

thread_local! {
    static GAUSSIAN_KERNEL_CACHE: RefCell<Vec<(u16, GaussianKernel)>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn with_gaussian_kernel<T>(radius: f64, work: impl FnOnce(&GaussianKernel) -> T) -> T {
    let key = (canonical_gaussian_radius(radius) * 4.0) as u16;
    GAUSSIAN_KERNEL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let index = cache
            .iter()
            .position(|(cached, _)| *cached == key)
            .unwrap_or_else(|| {
                if cache.len() == 16 {
                    cache.remove(0);
                }
                cache.push((key, GaussianKernel::new(canonical_gaussian_radius(radius))));
                cache.len() - 1
            });
        work(&cache[index].1)
    })
}

#[cfg(all(test, feature = "cpu"))]
pub(crate) fn gaussian_kernel_cache_len() -> usize {
    GAUSSIAN_KERNEL_CACHE.with(|cache| cache.borrow().len())
}

impl GaussianKernel {
    pub(crate) fn new(radius: f64) -> Self {
        let radius = radius.clamp(0.0, 32.0);
        let support = radius.ceil().max(1.0) as i32;
        let sigma = (radius / 3.0).max(0.5);
        let mut weights = Vec::with_capacity((support * 2 + 1) as usize);
        let mut sum = 0.0;
        for offset in -support..=support {
            let weight = (-0.5 * (f64::from(offset) / sigma).powi(2)).exp();
            weights.push(weight);
            sum += weight;
        }
        for weight in &mut weights {
            *weight /= sum;
        }
        let mut integer_weights = [0; 33];
        for offset in 1..=support as usize {
            integer_weights[offset] = (weights[support as usize + offset] * 65536.0).round() as u32;
        }
        integer_weights[0] = 65536 - 2 * integer_weights[1..].iter().sum::<u32>();
        Self {
            #[cfg(feature = "cpu")]
            weights,
            integer_weights,
            radius: support,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_weights_are_normalized_symmetric_and_bound_byte_accumulation() {
        for quarter in 0..=128 {
            let kernel = GaussianKernel::new(f64::from(quarter) / 4.0);
            assert_eq!(
                kernel.integer_weights[0] + 2 * kernel.integer_weights[1..].iter().sum::<u32>(),
                65536
            );
            assert!(
                kernel.integer_weights[kernel.radius as usize + 1..]
                    .iter()
                    .all(|&w| w == 0)
            );
        }
        assert!(65536_u64 * 255 * 255 + 65536 * 255 / 2 < u64::from(u32::MAX));
    }
}
