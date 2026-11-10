use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    Category, Diagnostic,
    project::{Asset, AssetType},
};
use vestra_media::{VideoMediaInfo, probe_audio_duration_cancellable, probe_video};

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
    cancelled: &AtomicBool,
) -> ValidatedAssets {
    let mut paths = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut audio_durations = BTreeMap::new();
    let mut video_durations = BTreeMap::new();
    let mut video_metadata = BTreeMap::new();
    let mut audio_probes = BTreeMap::new();
    for (index, asset) in assets.iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
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
                    AssetType::Audio => match audio_probes
                        .entry(resolved.clone())
                        .or_insert_with(|| probe_audio_duration_cancellable(&resolved, cancelled))
                    {
                        Ok(duration) => {
                            audio_durations.insert(asset.id.clone(), *duration);
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

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use super::*;

    #[test]
    fn shared_audio_paths_are_probed_once_per_preflight() {
        struct CountAudioProbes(Arc<AtomicUsize>);

        impl io::Write for CountAudioProbes {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if String::from_utf8_lossy(bytes).contains("audio media probe completed") {
                    self.0.fetch_add(1, Ordering::Relaxed);
                }
                Ok(bytes.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let assets = serde_json::from_value::<Vec<Asset>>(serde_json::json!([
            {"id": "first", "type": "audio", "source": "tone.wav"},
            {"id": "second", "type": "audio", "source": "tone.wav"}
        ]))
        .expect("assets");
        let probes = Arc::new(AtomicUsize::new(0));
        let writer_probes = Arc::clone(&probes);
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(move || CountAudioProbes(Arc::clone(&writer_probes)))
            .finish();
        let mut errors = Vec::new();
        let validated = tracing::subscriber::with_default(subscriber, || {
            validate(
                &assets,
                Path::new(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../assets/fixtures"
                )),
                &mut errors,
                &AtomicBool::new(false),
            )
        });

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(validated.audio_durations.len(), 2);
        assert!(validated.audio_durations["first"] > 0.0);
        assert_eq!(
            validated.audio_durations["first"],
            validated.audio_durations["second"]
        );
        assert_eq!(probes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn shared_invalid_audio_preserves_each_asset_diagnostic() {
        let directory = tempfile::tempdir().expect("temporary directory");
        std::fs::write(directory.path().join("broken.wav"), b"not audio").expect("invalid fixture");
        let assets = serde_json::from_value::<Vec<Asset>>(serde_json::json!([
            {"id": "first", "type": "audio", "source": "broken.wav"},
            {"id": "second", "type": "audio", "source": "broken.wav"}
        ]))
        .expect("assets");
        let mut errors = Vec::new();
        let validated = validate(
            &assets,
            directory.path(),
            &mut errors,
            &AtomicBool::new(false),
        );

        assert!(validated.audio_durations.is_empty());
        assert_eq!(errors.len(), 2);
        for (index, id) in ["first", "second"].iter().enumerate() {
            assert_eq!(errors[index].code, "VESTRA-ASSET-AUDIO");
            assert_eq!(
                errors[index].pointer.as_deref(),
                Some(format!("/assets/{index}/source").as_str())
            );
            assert_eq!(errors[index].related_id.as_deref(), Some(*id));
        }
    }
}
