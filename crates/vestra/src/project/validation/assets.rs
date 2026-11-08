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
    pub audio_duration_sources: BTreeMap<String, vestra_media::AudioDurationSource>,
    pub video_durations: BTreeMap<String, f64>,
    pub video_metadata: BTreeMap<String, VideoMediaInfo>,
}

pub(crate) fn validate(
    assets: &[Asset],
    root: &Path,
    errors: &mut Vec<Diagnostic>,
    cancelled: &AtomicBool,
    strict_audio: bool,
    audio: Option<&crate::project::AudioTimeline>,
) -> ValidatedAssets {
    let mut paths = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    let mut audio_durations = BTreeMap::new();
    let mut audio_duration_sources = BTreeMap::new();
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
                    AssetType::Audio => {
                        let result = audio_probes.entry(resolved.clone()).or_insert_with(|| {
                            let info = if strict_audio {
                                vestra_media::AudioMediaInfo {
                                    duration_seconds: probe_audio_duration_cancellable(
                                        &resolved, cancelled,
                                    )?,
                                    duration_source: vestra_media::AudioDurationSource::Decoded,
                                }
                            } else {
                                vestra_media::inspect_audio_cancellable(&resolved, cancelled)?
                            };
                            Ok::<_, vestra_media::MediaError>(info)
                        });
                        if let Ok(info) = result {
                            let exceeds_reported = audio
                                .into_iter()
                                .flat_map(|timeline| &timeline.tracks)
                                .flat_map(|track| &track.clips)
                                .any(|clip| {
                                    clip.asset == asset.id
                                        && (clip.trim_start >= info.duration_seconds
                                            || clip
                                                .trim_end
                                                .is_some_and(|end| end > info.duration_seconds))
                                });
                            if info.duration_source == vestra_media::AudioDurationSource::Reported
                                && exceeds_reported
                            {
                                *result = probe_audio_duration_cancellable(&resolved, cancelled)
                                    .map(|duration_seconds| vestra_media::AudioMediaInfo {
                                        duration_seconds,
                                        duration_source: vestra_media::AudioDurationSource::Decoded,
                                    });
                            }
                        }
                    }
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
    // A later alias may require exact bounds; all aliases use the final shared result.
    for (index, asset) in assets.iter().enumerate() {
        if asset.kind != AssetType::Audio {
            continue;
        }
        let Some(result) = paths.get(&asset.id).and_then(|path| audio_probes.get(path)) else {
            continue;
        };
        match result {
            Ok(info) => {
                audio_durations.insert(asset.id.clone(), info.duration_seconds);
                audio_duration_sources.insert(asset.id.clone(), info.duration_source);
            }
            Err(error) => errors.push(
                Diagnostic::error(
                    "VESTRA-ASSET-AUDIO",
                    Category::Media,
                    format!("invalid audio asset '{}': {error}", asset.id),
                    format!("/assets/{index}/source"),
                )
                .with_related_id(&asset.id),
            ),
        }
    }
    ValidatedAssets {
        paths,
        kinds,
        audio_durations,
        audio_duration_sources,
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
                    "/../../examples/assets"
                )),
                &mut errors,
                &AtomicBool::new(false),
                false,
                None,
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
            false,
            None,
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

#[cfg(test)]
mod metadata_bounds_tests {
    use super::*;

    #[test]
    fn underestimated_metadata_upgrades_all_aliases_and_later_operations_observe_edits() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("source.flac");
        let output = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=sample_rate=48000:duration=0.375",
                "-c:a",
                "flac",
            ])
            .arg(&path)
            .output()
            .expect("encode fixture");
        assert!(output.status.success());
        let mut bytes = std::fs::read(&path).expect("FLAC");
        let field = u64::from_be_bytes(bytes[18..26].try_into().expect("STREAMINFO"));
        bytes[18..26].copy_from_slice(&((field & !((1_u64 << 36) - 1)) | 6_000).to_be_bytes());
        std::fs::write(&path, &bytes).expect("underestimated duration");
        let assets: Vec<Asset> = serde_json::from_value(serde_json::json!([
            {"id":"first","type":"audio","source":"source.flac"},
            {"id":"second","type":"audio","source":"source.flac"}
        ]))
        .expect("assets");
        let audio = serde_json::from_value(serde_json::json!({"tracks":[{"id":"track","clips":[
            {"id":"clip","asset":"second","start":0,"trim_start":0,"trim_end":0.25}
        ]}]}))
        .expect("audio");
        let mut errors = Vec::new();
        let validated = validate(
            &assets,
            directory.path(),
            &mut errors,
            &AtomicBool::new(false),
            false,
            Some(&audio),
        );
        assert!(errors.is_empty(), "{errors:?}");
        for id in ["first", "second"] {
            assert_eq!(validated.audio_durations[id], 0.375);
            assert_eq!(
                validated.audio_duration_sources[id],
                vestra_media::AudioDurationSource::Decoded
            );
        }
        let validated = validate(
            &assets,
            directory.path(),
            &mut errors,
            &AtomicBool::new(false),
            false,
            None,
        );
        assert_eq!(validated.audio_durations["first"], 0.125);
        assert_eq!(
            validated.audio_duration_sources["first"],
            vestra_media::AudioDurationSource::Reported
        );
        std::fs::write(&path, b"changed invalid source").expect("source edit");
        let validated = validate(
            &assets,
            directory.path(),
            &mut errors,
            &AtomicBool::new(false),
            false,
            None,
        );
        assert!(validated.audio_durations.is_empty());
        assert_eq!(errors.len(), 2);
    }
}
