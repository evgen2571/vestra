use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{
    Category, Diagnostic,
    project::{Asset, AssetType},
};
use vestra_media::{VideoMediaInfo, probe_audio_duration, probe_video};

pub(crate) struct ValidatedAssets {
    pub paths: BTreeMap<String, PathBuf>,
    pub kinds: BTreeMap<String, AssetType>,
    pub audio_durations: BTreeMap<String, f64>,
    pub video_durations: BTreeMap<String, f64>,
    pub video_metadata: BTreeMap<String, VideoMediaInfo>,
}

pub(crate) fn validate(
    assets: &[Asset],
    root: &Path,
    errors: &mut Vec<Diagnostic>,
) -> ValidatedAssets {
    let mut paths = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut audio_durations = BTreeMap::new();
    let mut video_durations = BTreeMap::new();
    let mut video_metadata = BTreeMap::new();
    for (index, asset) in assets.iter().enumerate() {
        let pointer = format!("/assets/{index}");
        match super::super::paths::resolve_regular_file(root, &asset.source) {
            Ok(resolved) => {
                match asset.kind {
                    AssetType::Image => {
                        if let Err(error) = image::image_dimensions(&resolved) {
                            errors.push(
                                Diagnostic::error(
                                    "VESTRA-ASSET-IMAGE",
                                    Category::Media,
                                    format!("invalid image asset '{}': {error}", asset.id),
                                    format!("{pointer}/source"),
                                )
                                .with_related_id(&asset.id),
                            );
                        }
                    }
                    AssetType::Audio => match probe_audio_duration(&resolved) {
                        Ok(duration) => {
                            audio_durations.insert(asset.id.clone(), duration);
                        }
                        Err(error) => errors.push(
                            Diagnostic::error(
                                "VESTRA-ASSET-AUDIO",
                                Category::Media,
                                format!("invalid audio asset '{}': {error}", asset.id),
                                format!("{pointer}/source"),
                            )
                            .with_related_id(&asset.id),
                        ),
                    },
                    AssetType::Video => match probe_video(&resolved) {
                        Ok(info) => match info.duration_seconds {
                            Some(duration) if duration.is_finite() && duration > 0.0 => {
                                video_durations.insert(asset.id.clone(), duration);
                                video_metadata.insert(asset.id.clone(), info);
                            }
                            _ => errors.push(
                                Diagnostic::error(
                                    "VESTRA-ASSET-VIDEO-DURATION",
                                    Category::Media,
                                    format!("video asset '{}' has no finite duration", asset.id),
                                    format!("{pointer}/source"),
                                )
                                .with_related_id(&asset.id),
                            ),
                        },
                        Err(error) => errors.push(
                            Diagnostic::error(
                                "VESTRA-ASSET-VIDEO",
                                Category::Media,
                                format!("invalid video asset '{}': {error}", asset.id),
                                format!("{pointer}/source"),
                            )
                            .with_related_id(&asset.id),
                        ),
                    },
                    AssetType::Font => {
                        if std::fs::metadata(&resolved).is_ok_and(|metadata| metadata.len() == 0) {
                            errors.push(
                                Diagnostic::error(
                                    "VESTRA-ASSET-FONT",
                                    Category::Media,
                                    format!("invalid font asset '{}': file is empty", asset.id),
                                    format!("{pointer}/source"),
                                )
                                .with_related_id(&asset.id),
                            );
                        }
                    }
                }
                paths.insert(asset.id.clone(), resolved);
            }
            Err(error) => errors.push(
                Diagnostic::error(
                    "VESTRA-ASSET-PATH",
                    Category::Asset,
                    error,
                    format!("{pointer}/source"),
                )
                .with_related_id(&asset.id),
            ),
        }
        kinds.insert(asset.id.clone(), asset.kind);
    }
    ValidatedAssets {
        paths,
        kinds,
        audio_durations,
        video_durations,
        video_metadata,
    }
}
