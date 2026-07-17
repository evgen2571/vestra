use std::collections::{BTreeMap, VecDeque};

use image::{RgbaImage, imageops::FilterType};

use crate::{
    Category, Diagnostic,
    plan::{CompiledClip, CompiledSizing, PreparationClass, RenderPlan},
    project::Crop,
};

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct PreparationStats {
    pub decoded_image_count: usize,
    pub static_prepared_clip_count: usize,
    pub static_crop_count: usize,
    pub static_resize_count: usize,
    pub dynamic_clip_count: usize,
    pub bitmap_cache_hits: u64,
    pub bitmap_cache_misses: u64,
    pub peak_cache_entries: usize,
}

pub struct PreparedAssets {
    decoded: Vec<RgbaImage>,
    static_clips: Vec<Option<RgbaImage>>,
    dynamic_cache: BTreeMap<BitmapKey, RgbaImage>,
    cache_order: VecDeque<BitmapKey>,
    stats: PreparationStats,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BitmapKey {
    asset: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    target_width: u32,
    target_height: u32,
}

impl PreparedAssets {
    pub fn build(plan: &RenderPlan) -> Result<Self, Diagnostic> {
        let mut decoded = Vec::with_capacity(plan.images.len());
        for image_asset in &plan.images {
            let image = image::open(&image_asset.path).map_err(|error| {
                Diagnostic::error(
                    "MVP-IMAGE-DECODE",
                    Category::Media,
                    format!("cannot decode image '{}': {error}", image_asset.id),
                    "",
                )
            })?;
            decoded.push(image.to_rgba8());
        }
        let mut prepared = Self {
            static_clips: vec![None; plan.clips.len()],
            decoded,
            dynamic_cache: BTreeMap::new(),
            cache_order: VecDeque::new(),
            stats: PreparationStats::default(),
        };
        prepared.stats.decoded_image_count = prepared.decoded.len();
        for (index, clip) in plan.clips.iter().enumerate() {
            if matches!(
                clip.preparation,
                PreparationClass::StaticBitmap | PreparationClass::PositionOrOpacityOnly
            ) {
                prepared.stats.static_crop_count += 1;
                prepared.stats.static_resize_count += 1;
                let image = prepare_bitmap(
                    plan,
                    &prepared.decoded[clip.asset_index],
                    clip,
                    clip.crop,
                    1.0,
                );
                prepared.static_clips[index] = Some(image);
                prepared.stats.static_prepared_clip_count += 1;
            } else {
                prepared.stats.dynamic_clip_count += 1;
            }
        }
        Ok(prepared)
    }

    pub fn bitmap_for(
        &mut self,
        plan: &RenderPlan,
        clip_index: usize,
        crop: Crop,
        scale: f64,
    ) -> &RgbaImage {
        if let Some(image) = self.static_clips[clip_index].as_ref() {
            return image;
        }
        let clip = &plan.clips[clip_index];
        let source = &self.decoded[clip.asset_index];
        let (x, y, width, height) = crop_rect(source, crop);
        let (base_width, base_height) = sizing_dimensions(
            &clip.sizing,
            width,
            height,
            plan.canvas.width,
            plan.canvas.height,
        );
        let target_width = (f64::from(base_width) * scale).round().max(1.0) as u32;
        let target_height = (f64::from(base_height) * scale).round().max(1.0) as u32;
        let key = BitmapKey {
            asset: clip.asset_index,
            x,
            y,
            width,
            height,
            target_width,
            target_height,
        };
        if self.dynamic_cache.contains_key(&key) {
            self.stats.bitmap_cache_hits += 1;
            return self.dynamic_cache.get(&key).expect("cache key checked");
        }
        self.stats.bitmap_cache_misses += 1;
        let bitmap = prepare_bitmap(plan, &self.decoded[clip.asset_index], clip, crop, scale);
        if self.dynamic_cache.len() == 128 {
            if let Some(evicted) = self.cache_order.pop_front() {
                self.dynamic_cache.remove(&evicted);
            }
        }
        self.cache_order.push_back(key.clone());
        self.dynamic_cache.insert(key.clone(), bitmap);
        self.stats.peak_cache_entries = self.stats.peak_cache_entries.max(self.dynamic_cache.len());
        self.dynamic_cache.get(&key).expect("bitmap inserted")
    }

    #[must_use]
    pub fn stats(&self) -> &PreparationStats {
        &self.stats
    }
}

fn prepare_bitmap(
    plan: &RenderPlan,
    source: &RgbaImage,
    clip: &CompiledClip,
    crop: Crop,
    scale: f64,
) -> RgbaImage {
    let (x, y, width, height) = crop_rect(source, crop);
    let cropped = image::imageops::crop_imm(source, x, y, width, height).to_image();
    let (base_width, base_height) = sizing_dimensions(
        &clip.sizing,
        width,
        height,
        plan.canvas.width,
        plan.canvas.height,
    );
    let target_width = (f64::from(base_width) * scale).round().max(1.0) as u32;
    let target_height = (f64::from(base_height) * scale).round().max(1.0) as u32;
    image::imageops::resize(&cropped, target_width, target_height, FilterType::Lanczos3)
}

fn crop_rect(source: &RgbaImage, crop: Crop) -> (u32, u32, u32, u32) {
    let x = (crop.x * f64::from(source.width())).floor() as u32;
    let y = (crop.y * f64::from(source.height())).floor() as u32;
    let right = ((crop.x + crop.width) * f64::from(source.width())).ceil() as u32;
    let bottom = ((crop.y + crop.height) * f64::from(source.height())).ceil() as u32;
    (x, y, right - x, bottom - y)
}

fn sizing_dimensions(
    sizing: &CompiledSizing,
    source_width: u32,
    source_height: u32,
    canvas_width: u32,
    canvas_height: u32,
) -> (u32, u32) {
    match sizing {
        CompiledSizing::Original => (source_width, source_height),
        CompiledSizing::Stretch { width, height } => (*width, *height),
        CompiledSizing::Scale(scale) => (
            (f64::from(source_width) * scale).round().max(1.0) as u32,
            (f64::from(source_height) * scale).round().max(1.0) as u32,
        ),
        CompiledSizing::Fit | CompiledSizing::Cover => {
            let horizontal = f64::from(canvas_width) / f64::from(source_width);
            let vertical = f64::from(canvas_height) / f64::from(source_height);
            let factor = if matches!(sizing, CompiledSizing::Fit) {
                horizontal.min(vertical)
            } else {
                horizontal.max(vertical)
            };
            (
                (f64::from(source_width) * factor).round().max(1.0) as u32,
                (f64::from(source_height) * factor).round().max(1.0) as u32,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan::{CompileOptions, compile},
        project::{ValidationOptions, load_and_validate},
    };

    #[test]
    fn prepares_static_clip_once() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/static-image.json"),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("valid project");
        let plan = compile(&validated, CompileOptions::default()).expect("plan");
        let prepared = PreparedAssets::build(&plan).expect("prepared assets");
        assert_eq!(prepared.stats().decoded_image_count, 1);
        assert_eq!(prepared.stats().static_prepared_clip_count, 1);
        assert_eq!(prepared.stats().static_crop_count, 1);
        assert_eq!(prepared.stats().static_resize_count, 1);
    }
}
