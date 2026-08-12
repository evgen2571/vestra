use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Category, Diagnostic,
    project::{Asset, AssetType},
};

pub(super) fn validate(
    assets: &[Asset],
    errors: &mut Vec<Diagnostic>,
) -> BTreeMap<String, AssetType> {
    let mut kinds = BTreeMap::new();
    let mut ids = BTreeSet::new();
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
        if asset.source.trim().is_empty() {
            errors.push(Diagnostic::error(
                "MVP-ASSET-PATH",
                Category::Asset,
                "asset source must not be empty",
                format!("{pointer}/source"),
            ));
        }
        kinds.insert(asset.id.clone(), asset.kind);
    }
    kinds
}
