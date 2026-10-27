//! Shared bounded Gaussian kernels and quantization-input integer weights.

use std::cell::RefCell;
use vestra_core::effects::canonical_gaussian_radius;

#[derive(Clone)]
pub struct GaussianKernel {
    pub(crate) weights: Vec<f64>,
    pub(crate) integer_weights: [u32; 33],
    pub(crate) radius: i32,
}

thread_local! {
    static GAUSSIAN_KERNEL_CACHE: RefCell<Vec<(u16, GaussianKernel)>> = const { RefCell::new(Vec::new()) };
}

pub fn with_gaussian_kernel<T>(radius: f64, work: impl FnOnce(&GaussianKernel) -> T) -> T {
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

#[cfg(test)]
pub(crate) fn gaussian_kernel_cache_len() -> usize {
    GAUSSIAN_KERNEL_CACHE.with(|cache| cache.borrow().len())
}

impl GaussianKernel {
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }
    pub fn integer_weights(&self) -> &[u32; 33] {
        &self.integer_weights
    }
    pub fn radius(&self) -> i32 {
        self.radius
    }
    pub fn new(radius: f64) -> Self {
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
    fn kernels_reuse_canonical_radii_and_keep_the_original_eviction_bound() {
        GAUSSIAN_KERNEL_CACHE.with(|cache| cache.borrow_mut().clear());
        with_gaussian_kernel(2.0, |first| {
            assert_eq!(first.weights.len(), (first.radius * 2 + 1) as usize);
            assert!((first.weights.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        });
        let after_first = gaussian_kernel_cache_len();
        with_gaussian_kernel(2.0, |_| {});
        assert_eq!(gaussian_kernel_cache_len(), after_first);
        for radius in 1..24 {
            with_gaussian_kernel(f64::from(radius), |_| {});
        }
        assert!(gaussian_kernel_cache_len() <= 16);
        GAUSSIAN_KERNEL_CACHE.with(|cache| {
            let cache = cache.borrow();
            assert_eq!(cache.len(), 16);
            assert_eq!(cache.first().unwrap().0, 8 * 4);
            assert_eq!(cache.last().unwrap().0, 23 * 4);
        });
    }

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
