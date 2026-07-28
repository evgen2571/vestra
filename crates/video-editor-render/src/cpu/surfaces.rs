//! Reusable CPU ping-pong surfaces independent of project effect types.

use image::{GenericImage, Rgba, RgbaImage};

pub(crate) struct EffectSurfacePool {
    first: RgbaImage,
    second: RgbaImage,
    horizontal: RgbaImage,
    first_is_current: bool,
}

impl EffectSurfacePool {
    #[must_use]
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            first: RgbaImage::new(width, height),
            second: RgbaImage::new(width, height),
            horizontal: RgbaImage::new(width, height),
            first_is_current: true,
        }
    }

    pub(super) fn resize(&mut self, width: u32, height: u32) {
        if self.first.width() != width || self.first.height() != height {
            self.first = RgbaImage::new(width, height);
            self.second = RgbaImage::new(width, height);
            self.horizontal = RgbaImage::new(width, height);
            self.first_is_current = true;
        }
    }

    pub(super) fn current(&mut self) -> &mut RgbaImage {
        if self.first_is_current {
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

    pub(super) fn run(&mut self, execute: impl FnOnce(&RgbaImage, &mut RgbaImage, &mut RgbaImage)) {
        if self.first_is_current {
            execute(&self.first, &mut self.second, &mut self.horizontal);
        } else {
            execute(&self.second, &mut self.first, &mut self.horizontal);
        }
        self.first_is_current = !self.first_is_current;
    }

    pub(super) fn begin_from(&mut self, source: &RgbaImage) {
        self.first
            .copy_from(source, 0, 0)
            .expect("matching effect surface dimensions");
        self.first_is_current = true;
    }

    pub(super) fn copy_to(&mut self, destination: &mut RgbaImage) {
        destination
            .copy_from(self.current(), 0, 0)
            .expect("matching effect surface dimensions");
    }
}
