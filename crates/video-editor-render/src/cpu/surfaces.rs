//! Reusable CPU ping-pong surfaces independent of project effect types.

use image::{GenericImage, Rgba, RgbaImage};

pub(crate) struct EffectSurfacePool {
    first: RgbaImage,
    second: RgbaImage,
    original: RgbaImage,
    first_is_current: bool,
    original_is_current: bool,
    allocations: u64,
    reuses: u64,
    copy_bytes: u64,
}

impl EffectSurfacePool {
    #[must_use]
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            first: RgbaImage::new(width, height),
            second: RgbaImage::new(width, height),
            original: RgbaImage::new(width, height),
            first_is_current: true,
            original_is_current: false,
            allocations: 3,
            reuses: 0,
            copy_bytes: 0,
        }
    }

    pub(super) fn resize(&mut self, width: u32, height: u32) {
        if self.first.width() != width || self.first.height() != height {
            self.first = RgbaImage::new(width, height);
            self.second = RgbaImage::new(width, height);
            self.original = RgbaImage::new(width, height);
            self.first_is_current = true;
            self.original_is_current = false;
            self.allocations += 3;
        }
    }

    pub(super) fn current(&mut self) -> &mut RgbaImage {
        if self.original_is_current {
            &mut self.original
        } else if self.first_is_current {
            &mut self.first
        } else {
            &mut self.second
        }
    }

    pub(super) fn clear(&mut self) {
        for pixel in self.current().pixels_mut() {
            *pixel = Rgba([0, 0, 0, 0]);
        }
    }

    pub(super) fn run(&mut self, execute: impl FnOnce(&RgbaImage, &mut RgbaImage, &RgbaImage)) {
        self.reuses += 1;
        if self.original_is_current {
            if self.first_is_current {
                execute(&self.original, &mut self.first, &self.original);
            } else {
                execute(&self.original, &mut self.second, &self.original);
            }
            self.original_is_current = false;
            return;
        }
        if self.first_is_current {
            execute(&self.first, &mut self.second, &self.original);
        } else {
            execute(&self.second, &mut self.first, &self.original);
        }
        self.first_is_current = !self.first_is_current;
    }

    /// Pins the current image in the retained `original` slot without copying pixels.
    /// The first subsequent pass consumes that pinned image and writes into the
    /// vacated ping-pong slot; later passes continue ping-ponging while `original`
    /// remains available to composite operations.
    pub(super) fn pin_current_as_original(&mut self) {
        if self.original_is_current {
            return;
        }
        if self.first_is_current {
            std::mem::swap(&mut self.first, &mut self.original);
        } else {
            std::mem::swap(&mut self.second, &mut self.original);
        }
        self.original_is_current = true;
    }

    pub(super) fn begin_from(&mut self, source: &RgbaImage) {
        self.first
            .copy_from(source, 0, 0)
            .expect("matching effect surface dimensions");
        self.copy_bytes += image_bytes(source);
        self.first_is_current = true;
        self.original_is_current = false;
    }

    pub(super) fn copy_to(&mut self, destination: &mut RgbaImage) {
        destination
            .copy_from(self.current(), 0, 0)
            .expect("matching effect surface dimensions");
        self.copy_bytes += image_bytes(destination);
    }

    pub(super) fn take_current(&mut self) -> RgbaImage {
        let replacement = RgbaImage::new(self.current().width(), self.current().height());
        self.allocations += 1;
        if self.first_is_current {
            std::mem::replace(&mut self.first, replacement)
        } else {
            std::mem::replace(&mut self.second, replacement)
        }
    }

    pub(super) fn stats(&self) -> SurfacePoolStats {
        SurfacePoolStats {
            allocations: self.allocations,
            reuses: self.reuses,
            retained_buffers: 3,
            retained_bytes: image_bytes(&self.first) * 3,
            copy_bytes: self.copy_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SurfacePoolStats {
    pub(super) allocations: u64,
    pub(super) reuses: u64,
    pub(super) retained_buffers: usize,
    pub(super) retained_bytes: u64,
    pub(super) copy_bytes: u64,
}

fn image_bytes(image: &RgbaImage) -> u64 {
    u64::from(image.width()) * u64::from(image.height()) * 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinning_current_as_original_reuses_retained_storage_without_copying_pixels() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
        let mut pool = EffectSurfacePool::new(2, 2);
        pool.begin_from(&source);
        let copied_before_pin = pool.stats().copy_bytes;

        pool.pin_current_as_original();
        assert_eq!(pool.stats().copy_bytes, copied_before_pin);
        pool.run(|current, target, original| {
            assert_eq!(current, original);
            target.copy_from(current, 0, 0).expect("matching surfaces");
        });

        assert_eq!(&*pool.current(), &source);
        assert_eq!(pool.stats().copy_bytes, copied_before_pin);
    }

    #[test]
    fn reuses_a_fixed_ping_pong_set_and_transfers_cached_output() {
        let mut pool = EffectSurfacePool::new(4, 2);
        pool.run(|source, target, _| target.copy_from(source, 0, 0).expect("matching surfaces"));
        pool.run(|source, target, _| target.copy_from(source, 0, 0).expect("matching surfaces"));
        assert_eq!(pool.stats().allocations, 3);
        assert_eq!(pool.stats().reuses, 2);

        let cached = pool.take_current();
        assert_eq!(cached.dimensions(), (4, 2));
        assert_eq!(pool.stats().allocations, 4);
        assert_eq!(pool.stats().retained_buffers, 3);
        assert_eq!(pool.stats().retained_bytes, 96);
    }
}
