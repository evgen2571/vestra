use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use crate::{
    Category, Diagnostic, media,
    project::{Asset, AssetType},
};

pub(crate) struct ValidatedAssets {
    pub paths: BTreeMap<String, PathBuf>,
    pub kinds: BTreeMap<String, AssetType>,
    pub audio_durations: BTreeMap<String, f64>,
}

pub(crate) fn validate(
    assets: &[Asset],
    root: &Path,
    errors: &mut Vec<Diagnostic>,
) -> ValidatedAssets {
    let mut paths = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut audio_durations = BTreeMap::new();
    for (index, asset) in assets.iter().enumerate() {
        let pointer = format!("/assets/{index}");
        if asset.id.trim().is_empty() {
            errors.push(Diagnostic::error(
                "MVP-ASSET-ID",
                Category::Semantic,
                "asset id must not be empty",
                format!("{pointer}/id"),
            ));
        }
        if !ids.insert(asset.id.clone()) {
            errors.push(
                Diagnostic::error(
                    "MVP-ASSET-DUPLICATE",
                    Category::Semantic,
                    format!("duplicate asset id '{}'", asset.id),
                    format!("{pointer}/id"),
                )
                .with_related_id(&asset.id),
            );
        }
        match super::super::paths::resolve_regular_file(root, &asset.source) {
            Ok(resolved) => {
                match asset.kind {
                    AssetType::Image => {
                        if let Err(error) = image::image_dimensions(&resolved) {
                            errors.push(
                                Diagnostic::error(
                                    "MVP-ASSET-IMAGE",
                                    Category::Media,
                                    format!("invalid image asset '{}': {error}", asset.id),
                                    format!("{pointer}/source"),
                                )
                                .with_related_id(&asset.id),
                            );
                        }
                    }
                    AssetType::Audio => match media::probe_audio_duration(&resolved) {
                        Ok(duration) => {
                            audio_durations.insert(asset.id.clone(), duration);
                        }
                        Err(error) => errors.push(
                            Diagnostic::error(
                                "MVP-ASSET-AUDIO",
                                Category::Media,
                                format!("invalid audio asset '{}': {error}", asset.id),
                                format!("{pointer}/source"),
                            )
                            .with_related_id(&asset.id),
                        ),
                    },
                }
                paths.insert(asset.id.clone(), resolved);
            }
            Err(error) => errors.push(
                Diagnostic::error(
                    "MVP-ASSET-PATH",
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
    }
}
