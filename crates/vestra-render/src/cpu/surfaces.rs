//! Reusable CPU effect surfaces with explicit logical resource bindings.

use image::{GenericImage, Rgba, RgbaImage};

use vestra_core::plan::EffectResource;

const SURFACE_COUNT: usize = 3;

/// Worker-local full-frame surfaces used for isolated nested compositions.
///
/// A surface is removed from the pool while it is live. This makes parent and
/// child compositions unable to alias, while sibling surfaces can be reused
/// once their result has been blended into the parent.
pub(crate) struct CompositionSurfacePool {
    surfaces: Vec<Option<RgbaImage>>,
    allocations: u64,
    reuses: u64,
}

impl CompositionSurfacePool {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            surfaces: Vec::new(),
            allocations: 0,
            reuses: 0,
        }
    }

    pub(crate) fn acquire(&mut self, depth: usize, width: u32, height: u32) -> RgbaImage {
        if self.surfaces.len() <= depth {
            self.surfaces.resize_with(depth + 1, || None);
        }
        let (mut surface, reused) = if let Some(surface) = self.surfaces[depth].take() {
            (surface, true)
        } else {
            self.allocations += 1;
            (RgbaImage::new(width, height), false)
        };
        if surface.width() != width || surface.height() != height {
            self.allocations += 1;
            surface = RgbaImage::new(width, height);
        } else if reused {
            self.reuses += 1;
        }
        for pixel in surface.pixels_mut() {
            *pixel = Rgba([0, 0, 0, 0]);
        }
        surface
    }

    pub(crate) fn release(&mut self, depth: usize, surface: RgbaImage) {
        let slot = self
            .surfaces
            .get_mut(depth)
            .expect("released composition depth was not acquired");
        assert!(slot.is_none(), "composition surface depth is still live");
        *slot = Some(surface);
    }

    #[cfg(test)]
    pub(crate) fn stats(&self) -> (u64, u64, usize) {
        (self.allocations, self.reuses, self.surfaces.len())
    }
}

pub(crate) struct EffectSurfacePool {
    surfaces: [RgbaImage; SURFACE_COUNT],
    mask_local_surface: RgbaImage,
    mask_surface: RgbaImage,
    mask_coverage: Vec<f32>,
    mask_feather_scratch: Vec<f32>,
    mask_feather_scratch_b: Vec<f32>,
    current_slot: usize,
    original_slot: Option<usize>,
    temporary_slots: [Option<usize>; 2],
    allocations: u64,
    reuses: u64,
    copy_bytes: u64,
}

impl EffectSurfacePool {
    #[must_use]
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            surfaces: std::array::from_fn(|_| RgbaImage::new(width, height)),
            mask_local_surface: RgbaImage::new(width, height),
            mask_surface: RgbaImage::new(width, height),
            mask_coverage: vec![1.0; (width as usize) * (height as usize)],
            mask_feather_scratch: vec![0.0; (width as usize) * (height as usize)],
            mask_feather_scratch_b: vec![0.0; (width as usize) * (height as usize)],
            current_slot: 0,
            original_slot: None,
            temporary_slots: [None; 2],
            allocations: SURFACE_COUNT as u64,
            reuses: 0,
            copy_bytes: 0,
        }
    }

    pub(super) fn resize(&mut self, width: u32, height: u32) {
        if self.surfaces[0].width() != width || self.surfaces[0].height() != height {
            self.surfaces = std::array::from_fn(|_| RgbaImage::new(width, height));
            self.mask_local_surface = RgbaImage::new(width, height);
            self.mask_surface = RgbaImage::new(width, height);
            self.mask_coverage = vec![1.0; (width as usize) * (height as usize)];
            self.mask_feather_scratch = vec![0.0; (width as usize) * (height as usize)];
            self.mask_feather_scratch_b = vec![0.0; (width as usize) * (height as usize)];
            self.current_slot = 0;
            self.original_slot = None;
            self.temporary_slots = [None; 2];
            self.allocations += SURFACE_COUNT as u64;
        }
    }

    pub(super) fn current(&mut self) -> &mut RgbaImage {
        &mut self.surfaces[self.current_slot]
    }

    pub(super) fn mask_local_surface(&mut self) -> &mut RgbaImage {
        &mut self.mask_local_surface
    }

    pub(super) fn copy_current_to_mask_local(&mut self) {
        let current_slot = self.current_slot;
        self.mask_local_surface
            .copy_from(&self.surfaces[current_slot], 0, 0)
            .expect("matching effect surface dimensions");
    }

    pub(super) fn apply_external_matte(
        &mut self,
        mode: crate::project::MaskCoverageMode,
        invert: bool,
    ) {
        let current_slot = self.current_slot;
        let (surfaces, matte) = (&mut self.surfaces, &self.mask_local_surface);
        for (pixel, matte_pixel) in surfaces[current_slot].pixels_mut().zip(matte.pixels()) {
            let mut coverage = crate::project::mask_coverage(matte_pixel.0, mode);
            if invert {
                coverage = 1.0 - coverage;
            }
            pixel[3] = (f32::from(pixel[3]) * coverage).round().clamp(0.0, 255.0) as u8;
        }
    }

    pub(super) fn reset_mask_coverage(&mut self) {
        self.mask_coverage.fill(1.0);
    }

    pub(super) fn combine_mask_coverage(
        &mut self,
        operation: crate::project::MaskOperation,
        invert: bool,
        strength: f32,
    ) {
        for (coverage, pixel) in self
            .mask_coverage
            .iter_mut()
            .zip(self.mask_surface.pixels())
        {
            *coverage = crate::project::apply_mask_operation(
                *coverage,
                f32::from(pixel[3]) / 255.0,
                operation,
                invert,
                strength,
            );
        }
    }

    /// Applies the shared three-pass separable box approximation to coverage.
    /// Samples outside the canvas are transparent, so feathering cannot wrap
    /// around an edge.
    pub(super) fn feather_mask_surface(&mut self, radius: f32) {
        if radius <= 0.0 {
            return;
        }
        let width = self.mask_surface.width() as usize;
        let height = self.mask_surface.height() as usize;
        let half_width = crate::project::mask_feather_box_half_width(radius);
        let mut source = &mut self.mask_feather_scratch;
        let mut destination = &mut self.mask_feather_scratch_b;
        for y in 0..height {
            for x in 0..width {
                source[y * width + x] =
                    f32::from(self.mask_surface.get_pixel(x as u32, y as u32)[3]) / 255.0;
            }
        }
        for _ in 0..crate::project::MASK_FEATHER_PASSES {
            Self::box_blur(source, destination, width, height, half_width, true);
            std::mem::swap(&mut source, &mut destination);
        }
        for _ in 0..crate::project::MASK_FEATHER_PASSES {
            Self::box_blur(source, destination, width, height, half_width, false);
            std::mem::swap(&mut source, &mut destination);
        }
        for (index, value) in source.iter().enumerate() {
            let x = index % width;
            let y = index / width;
            self.mask_surface.put_pixel(
                x as u32,
                y as u32,
                Rgba([0, 0, 0, (value * 255.0).round().clamp(0.0, 255.0) as u8]),
            );
        }
    }

    fn box_blur(
        source: &[f32],
        destination: &mut [f32],
        width: usize,
        height: usize,
        half_width: f32,
        horizontal: bool,
    ) {
        let extent = 2.0 * half_width + 1.0;
        for y in 0..height {
            for x in 0..width {
                let center = if horizontal { x as f32 } else { y as f32 };
                let first = (center - half_width - 0.5).floor() as i32;
                let last = (center + half_width + 0.5).ceil() as i32;
                let mut value = 0.0;
                for sample in first..last {
                    let overlap = (sample as f32 + 0.5).min(center + half_width + 0.5)
                        - (sample as f32 - 0.5).max(center - half_width - 0.5);
                    if overlap <= 0.0 {
                        continue;
                    }
                    let valid = if horizontal {
                        sample >= 0 && (sample as usize) < width
                    } else {
                        sample >= 0 && (sample as usize) < height
                    };
                    if valid {
                        let index = if horizontal {
                            y * width + sample as usize
                        } else {
                            sample as usize * width + x
                        };
                        value += source[index] * overlap;
                    }
                }
                destination[y * width + x] = value / extent;
            }
        }
    }

    pub(super) fn apply_mask_coverage(&mut self) {
        let current_slot = self.current_slot;
        let (surfaces, coverage) = (&mut self.surfaces, &self.mask_coverage);
        for (pixel, coverage) in surfaces[current_slot].pixels_mut().zip(coverage) {
            pixel[3] = (f32::from(pixel[3]) * coverage).round().clamp(0.0, 255.0) as u8;
        }
    }

    pub(super) fn compose_mask_surface(&mut self, transform: crate::animation::Transform2D) {
        for pixel in self.mask_surface.pixels_mut() {
            *pixel = image::Rgba([0, 0, 0, 0]);
        }
        super::raster::draw_surface(
            &mut self.mask_surface,
            &self.mask_local_surface,
            transform,
            vestra_core::plan::ColourTransform::default(),
        );
    }

    pub(super) fn clear(&mut self) {
        for pixel in self.current().pixels_mut() {
            *pixel = Rgba([0, 0, 0, 0]);
        }
    }

    /// Starts one effect plan. `Original` aliases the current surface until a
    /// pass writes a new `Current`, so retaining it never requires a frame copy.
    pub(super) fn begin_effect(&mut self, retain_original: bool) {
        self.original_slot = retain_original.then_some(self.current_slot);
        self.temporary_slots = [None; 2];
    }

    /// Executes one pass using the logical resource bindings declared by the IR.
    /// The physical destination is selected so no live input or aliased logical
    /// resource is overwritten.
    pub(super) fn run_pass(
        &mut self,
        primary: EffectResource,
        secondary: Option<EffectResource>,
        output: EffectResource,
        execute: impl FnOnce(&RgbaImage, Option<&RgbaImage>, &mut RgbaImage),
    ) {
        assert_ne!(
            output,
            EffectResource::Original,
            "effect passes cannot overwrite Original"
        );
        let primary_slot = self.resolve_slot(primary);
        let secondary_slot = secondary.map(|resource| self.resolve_slot(resource));
        let destination = self.destination_slot(output, primary_slot, secondary_slot);

        self.reuses += 1;
        with_sources_and_target(
            &mut self.surfaces,
            primary_slot,
            secondary_slot,
            destination,
            execute,
        );
        self.bind_output(output, destination);
    }

    /// Releases the physical binding for a dead logical temporary so a later
    /// pass can reuse that surface. Ordered-plan liveness is computed by the
    /// caller; `Current` and `Original` remain pinned by their dedicated state.
    pub(super) fn release_temporary(&mut self, resource: EffectResource) {
        match resource {
            EffectResource::Temporary0 => self.temporary_slots[0] = None,
            EffectResource::Temporary1 => self.temporary_slots[1] = None,
            EffectResource::Original | EffectResource::Current => {
                unreachable!("only temporary effect resources can be released")
            }
        }
    }

    fn resolve_slot(&self, resource: EffectResource) -> usize {
        match resource {
            EffectResource::Original => self
                .original_slot
                .expect("effect pass references Original without retaining it"),
            EffectResource::Current => self.current_slot,
            EffectResource::Temporary0 => self.temporary_slots[0]
                .expect("ordered effect pass references uninitialized Temporary0"),
            EffectResource::Temporary1 => self.temporary_slots[1]
                .expect("ordered effect pass references uninitialized Temporary1"),
        }
    }

    fn destination_slot(
        &self,
        output: EffectResource,
        primary_slot: usize,
        secondary_slot: Option<usize>,
    ) -> usize {
        if let Some(existing) = self.bound_slot(output)
            && existing != primary_slot
            && secondary_slot != Some(existing)
            && !self.is_aliased_by_other_resource(output, existing)
        {
            return existing;
        }

        (0..SURFACE_COUNT)
            .find(|&slot| {
                slot != primary_slot && secondary_slot != Some(slot) && !self.is_slot_live(slot)
            })
            .expect("effect pass plan requires more simultaneously live CPU surfaces")
    }

    fn bound_slot(&self, resource: EffectResource) -> Option<usize> {
        match resource {
            EffectResource::Original => self.original_slot,
            EffectResource::Current => Some(self.current_slot),
            EffectResource::Temporary0 => self.temporary_slots[0],
            EffectResource::Temporary1 => self.temporary_slots[1],
        }
    }

    fn is_slot_live(&self, slot: usize) -> bool {
        self.current_slot == slot
            || self.original_slot == Some(slot)
            || self.temporary_slots.contains(&Some(slot))
    }

    fn is_aliased_by_other_resource(&self, output: EffectResource, slot: usize) -> bool {
        [
            EffectResource::Original,
            EffectResource::Current,
            EffectResource::Temporary0,
            EffectResource::Temporary1,
        ]
        .into_iter()
        .any(|resource| resource != output && self.bound_slot(resource) == Some(slot))
    }

    fn bind_output(&mut self, output: EffectResource, slot: usize) {
        match output {
            EffectResource::Original => unreachable!("effect passes cannot overwrite Original"),
            EffectResource::Current => self.current_slot = slot,
            EffectResource::Temporary0 => self.temporary_slots[0] = Some(slot),
            EffectResource::Temporary1 => self.temporary_slots[1] = Some(slot),
        }
    }

    pub(super) fn begin_from(&mut self, source: &RgbaImage) {
        self.surfaces[0]
            .copy_from(source, 0, 0)
            .expect("matching effect surface dimensions");
        self.copy_bytes += image_bytes(source);
        self.current_slot = 0;
        self.original_slot = None;
        self.temporary_slots = [None; 2];
    }

    pub(super) fn copy_to(&mut self, destination: &mut RgbaImage) {
        destination
            .copy_from(&self.surfaces[self.current_slot], 0, 0)
            .expect("matching effect surface dimensions");
        self.copy_bytes += image_bytes(destination);
    }

    pub(super) fn take_current(&mut self) -> RgbaImage {
        let replacement = RgbaImage::new(
            self.surfaces[self.current_slot].width(),
            self.surfaces[self.current_slot].height(),
        );
        self.allocations += 1;
        std::mem::replace(&mut self.surfaces[self.current_slot], replacement)
    }

    pub(super) fn stats(&self) -> SurfacePoolStats {
        SurfacePoolStats {
            allocations: self.allocations,
            reuses: self.reuses,
            retained_buffers: SURFACE_COUNT,
            retained_bytes: image_bytes(&self.surfaces[0]) * SURFACE_COUNT as u64,
            copy_bytes: self.copy_bytes,
        }
    }
}

fn with_sources_and_target(
    surfaces: &mut [RgbaImage; SURFACE_COUNT],
    primary: usize,
    secondary: Option<usize>,
    destination: usize,
    execute: impl FnOnce(&RgbaImage, Option<&RgbaImage>, &mut RgbaImage),
) {
    debug_assert_ne!(primary, destination);
    debug_assert_ne!(secondary, Some(destination));
    let (before, target_and_after) = surfaces.split_at_mut(destination);
    let (target, after) = target_and_after
        .split_first_mut()
        .expect("destination is a valid retained surface");
    let resolve = |slot: usize| -> &RgbaImage {
        if slot < destination {
            &before[slot]
        } else {
            &after[slot - destination - 1]
        }
    };
    execute(resolve(primary), secondary.map(resolve), target);
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
    fn composition_surfaces_clear_on_reuse_and_grow_by_depth() {
        let mut pool = CompositionSurfacePool::new();
        let mut parent = pool.acquire(0, 2, 2);
        parent.put_pixel(0, 0, Rgba([10, 20, 30, 255]));
        let child = pool.acquire(1, 2, 2);
        assert_eq!(child.get_pixel(0, 0), &Rgba([0, 0, 0, 0]));
        pool.release(1, child);
        pool.release(0, parent);

        let reused = pool.acquire(0, 2, 2);
        assert_eq!(reused.get_pixel(0, 0), &Rgba([0, 0, 0, 0]));
        let (allocations, reuses, retained_depths) = pool.stats();
        assert_eq!(allocations, 2);
        assert_eq!(reuses, 1);
        assert_eq!(retained_depths, 2);
    }

    fn copy_pass(pool: &mut EffectSurfacePool, input: EffectResource, output: EffectResource) {
        pool.run_pass(input, None, output, |source, _, target| {
            target.copy_from(source, 0, 0).expect("matching surfaces");
        });
    }

    #[test]
    fn original_aliases_current_without_copy_and_survives_current_rebinding() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
        let mut pool = EffectSurfacePool::new(2, 2);
        pool.begin_from(&source);
        let copied_before_pin = pool.stats().copy_bytes;

        pool.begin_effect(true);
        pool.run_pass(
            EffectResource::Current,
            None,
            EffectResource::Current,
            |current, _, target| {
                target.copy_from(current, 0, 0).expect("matching surfaces");
                target.put_pixel(0, 0, Rgba([99, 20, 30, 255]));
            },
        );
        pool.run_pass(
            EffectResource::Original,
            None,
            EffectResource::Temporary0,
            |original, _, target| {
                target.copy_from(original, 0, 0).expect("matching surfaces");
            },
        );

        assert_eq!(pool.stats().copy_bytes, copied_before_pin);
        assert_eq!(pool.current().get_pixel(0, 0), &Rgba([99, 20, 30, 255]));
        let original_slot = pool.resolve_slot(EffectResource::Temporary0);
        assert_eq!(
            pool.surfaces[original_slot].get_pixel(0, 0),
            &Rgba([10, 20, 30, 255])
        );
    }

    #[test]
    fn current_and_temporaries_remain_independent_logical_resources() {
        let source = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 255]));
        let mut pool = EffectSurfacePool::new(1, 1);
        pool.begin_from(&source);
        pool.begin_effect(false);

        pool.run_pass(
            EffectResource::Current,
            None,
            EffectResource::Temporary0,
            |source, _, target| {
                target.copy_from(source, 0, 0).expect("matching surfaces");
                target.put_pixel(0, 0, Rgba([100, 20, 30, 255]));
            },
        );
        pool.run_pass(
            EffectResource::Current,
            None,
            EffectResource::Temporary1,
            |source, _, target| {
                target.copy_from(source, 0, 0).expect("matching surfaces");
                target.put_pixel(0, 0, Rgba([10, 110, 30, 255]));
            },
        );

        assert_eq!(pool.current().get_pixel(0, 0), &Rgba([10, 20, 30, 255]));
        assert_ne!(
            pool.resolve_slot(EffectResource::Temporary0),
            pool.resolve_slot(EffectResource::Temporary1)
        );

        copy_pass(
            &mut pool,
            EffectResource::Temporary0,
            EffectResource::Current,
        );
        assert_eq!(pool.current().get_pixel(0, 0), &Rgba([100, 20, 30, 255]));
    }

    #[test]
    fn reuses_a_fixed_resource_set_and_transfers_cached_output() {
        let mut pool = EffectSurfacePool::new(4, 2);
        pool.begin_effect(false);
        copy_pass(&mut pool, EffectResource::Current, EffectResource::Current);
        copy_pass(&mut pool, EffectResource::Current, EffectResource::Current);
        assert_eq!(pool.stats().allocations, 3);
        assert_eq!(pool.stats().reuses, 2);

        let cached = pool.take_current();
        assert_eq!(cached.dimensions(), (4, 2));
        assert_eq!(pool.stats().allocations, 4);
        assert_eq!(pool.stats().retained_buffers, 3);
        assert_eq!(pool.stats().retained_bytes, 96);
    }

    #[test]
    fn mask_feather_blurs_coverage_without_wrapping_edges() {
        let mut pool = EffectSurfacePool::new(5, 1);
        pool.mask_surface.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        pool.mask_surface.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        pool.feather_mask_surface(1.0);
        assert!(pool.mask_surface.get_pixel(0, 0)[3] < 255);
        assert!(pool.mask_surface.get_pixel(4, 0)[3] < 1);
    }

    #[test]
    fn mask_feather_has_a_smooth_monotonic_edge() {
        let mut pool = EffectSurfacePool::new(65, 65);
        for y in 0..65 {
            for x in 32..65 {
                pool.mask_surface.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        pool.feather_mask_surface(3.0);
        let values = (0..65)
            .map(|x| f32::from(pool.mask_surface.get_pixel(x, 32)[3]) / 255.0)
            .collect::<Vec<_>>();
        assert!(values[..33].windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(values[35..].windows(2).all(|pair| pair[0] >= pair[1]));
        assert!(values[0] < values[31]);
        assert!(values[32] > values[64]);
    }

    #[test]
    fn large_feather_has_many_distinct_levels_without_secondary_edges() {
        let mut pool = EffectSurfacePool::new(257, 257);
        for y in 0..257 {
            for x in 128..257 {
                pool.mask_surface.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        pool.feather_mask_surface(64.0);
        let values = (0..257)
            .map(|x| f32::from(pool.mask_surface.get_pixel(x, 128)[3]) / 255.0)
            .collect::<Vec<_>>();
        assert!(values[..160].windows(2).all(|pair| pair[0] <= pair[1]));
        let levels = (80..176)
            .map(|x| pool.mask_surface.get_pixel(x, 128)[3])
            .collect::<std::collections::BTreeSet<_>>();
        assert!(levels.len() > 12);
    }
}
