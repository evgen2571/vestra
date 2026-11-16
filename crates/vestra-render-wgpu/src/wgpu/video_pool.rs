//! Adapter-independent video decoder ownership and request sharing.
use crate::Diagnostic;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use vestra_core::plan::VideoAsset;
use vestra_render_common::{
    VideoDecoderFactory, VideoDecoderMetrics, VideoDecoderSession, VideoFrame,
};

pub(super) struct VideoPool {
    video_decoders: BTreeMap<usize, VideoAssetDecoders>,
    retired_video_metrics: VideoDecoderMetrics,
    video_decoder_open_count: u64,
}

struct VideoAssetDecoders {
    asset: VideoAsset,
    factory: Arc<dyn VideoDecoderFactory>,
    sessions: Vec<VideoSession>,
}

struct VideoSession {
    source_index: Option<usize>,
    decoder: Box<dyn VideoDecoderSession>,
}

impl VideoPool {
    pub(super) fn new(
        assets: &[VideoAsset],
        slot_assets: &[usize],
        cache_budget_bytes: u64,
        factory: Option<Arc<dyn VideoDecoderFactory>>,
    ) -> Result<(Self, BTreeMap<usize, VideoFrame>), Diagnostic> {
        let mut video_decoders = BTreeMap::new();
        let mut video_decoder_open_count = 0;
        let mut initial_video_frames = BTreeMap::new();
        if !slot_assets.is_empty() && factory.is_none() {
            return Err(Diagnostic::error(
                "WGPU-VIDEO-FACTORY",
                crate::Category::Media,
                "compiled video layers require a video decoder factory",
                "",
            ));
        }
        let used_video_assets = slot_assets.iter().copied().collect::<BTreeSet<_>>();
        if let Some(factory) = factory {
            let per_decoder_budget = cache_budget_bytes
                .checked_div(used_video_assets.len().max(1) as u64)
                .unwrap_or(0);
            for asset_index in used_video_assets {
                let asset = assets.get(asset_index).ok_or_else(|| {
                    Diagnostic::error(
                        "WGPU-VIDEO-ASSET",
                        crate::Category::Media,
                        "compiled video slot references a missing video asset",
                        "",
                    )
                })?;
                video_decoder_open_count += 1;
                let mut decoder = factory
                    .open_with_span(asset, per_decoder_budget, tracing::Span::current())
                    .map_err(|error| {
                        Diagnostic::error("WGPU-VIDEO-OPEN", crate::Category::Media, error, "")
                    })?;
                decoder.enable_prefetch();
                let frame = decoder
                    .frame_at_with_span(0.0, tracing::Span::current())
                    .map_err(|error| {
                        Diagnostic::error("WGPU-VIDEO-DECODE", crate::Category::Media, error, "")
                    })?;
                initial_video_frames.insert(asset_index, frame);
                video_decoders.insert(
                    asset_index,
                    VideoAssetDecoders {
                        asset: asset.clone(),
                        factory: Arc::clone(&factory),
                        sessions: vec![VideoSession {
                            source_index: None,
                            decoder,
                        }],
                    },
                );
            }
        }
        Ok((
            Self {
                video_decoders,
                retired_video_metrics: VideoDecoderMetrics::default(),
                video_decoder_open_count,
            },
            initial_video_frames,
        ))
    }

    pub(super) fn frames(
        &mut self,
        requested: BTreeMap<usize, (usize, f64)>,
    ) -> Result<Vec<(usize, VideoFrame)>, Diagnostic> {
        // Preserve live slot bindings before reusing an idle primary cursor.
        for pool in self.video_decoders.values_mut() {
            let mut index = 0;
            pool.sessions.retain_mut(|session| {
                let active = session
                    .source_index
                    .is_some_and(|slot| requested.contains_key(&slot));
                let keep = active || index == 0;
                index += 1;
                if !keep {
                    let metrics = session.decoder.metrics();
                    self.retired_video_metrics.frame_requests += metrics.frame_requests;
                    self.retired_video_metrics.actual_decodes += metrics.actual_decodes;
                    self.retired_video_metrics.seeks += metrics.seeks;
                    self.retired_video_metrics.cache_hits += metrics.cache_hits;
                    self.retired_video_metrics.cache_misses += metrics.cache_misses;
                    self.retired_video_metrics.decode_time_us += metrics.decode_time_us;
                    self.retired_video_metrics.cache_bytes_peak += metrics.cache_bytes_peak;
                } else if !active {
                    session.source_index = None;
                }
                keep
            });
        }
        // Share only immutable results within this submission, never cursor ownership.
        let mut frames = BTreeMap::new();
        let mut decoded = Vec::with_capacity(requested.len());
        for (source_index, (asset_index, source_time)) in requested {
            let key = (asset_index, source_time.to_bits());
            let frame = if let Some(frame) = frames.get(&key) {
                frame
            } else {
                let pool = self.video_decoders.get_mut(&asset_index).ok_or_else(|| {
                    Diagnostic::error(
                        "WGPU-VIDEO-DECODER",
                        crate::Category::Media,
                        "video decoder session is missing",
                        "",
                    )
                })?;
                let available = pool
                    .sessions
                    .iter()
                    .position(|session| session.source_index == Some(source_index))
                    .or_else(|| {
                        pool.sessions
                            .iter()
                            .position(|session| session.source_index.is_none())
                    });
                let index = if let Some(index) = available {
                    index
                } else {
                    let mut decoder = pool
                        .factory
                        .open_with_span(&pool.asset, 0, tracing::Span::current())
                        .map_err(|error| {
                            Diagnostic::error("WGPU-VIDEO-OPEN", crate::Category::Media, error, "")
                        })?;
                    decoder.enable_prefetch();
                    pool.sessions.push(VideoSession {
                        source_index: None,
                        decoder,
                    });
                    self.video_decoder_open_count += 1;
                    pool.sessions.len() - 1
                };
                let session = &mut pool.sessions[index];
                session.source_index = Some(source_index);
                let frame = session
                    .decoder
                    .frame_at_with_span(source_time, tracing::Span::current())
                    .map_err(|error| {
                        Diagnostic::error("WGPU-VIDEO-DECODE", crate::Category::Media, error, "")
                    })?;
                frames.entry(key).or_insert(frame)
            };
            decoded.push((source_index, frame.clone()));
        }
        Ok(decoded)
    }

    pub(super) fn session_count(&self) -> usize {
        self.video_decoders
            .values()
            .map(|pool| pool.sessions.len())
            .sum()
    }

    pub(super) fn open_count(&self) -> u64 {
        self.video_decoder_open_count
    }

    pub(super) fn metrics(&self) -> VideoDecoderMetrics {
        let mut metrics = self.retired_video_metrics;
        for session in self.video_decoders.values().flat_map(|pool| &pool.sessions) {
            let current = session.decoder.metrics();
            metrics.frame_requests += current.frame_requests;
            metrics.actual_decodes += current.actual_decodes;
            metrics.seeks += current.seeks;
            metrics.cache_hits += current.cache_hits;
            metrics.cache_misses += current.cache_misses;
            metrics.decode_time_us += current.decode_time_us;
            metrics.cache_bytes_retained += current.cache_bytes_retained;
            metrics.cache_bytes_peak += current.cache_bytes_peak;
            metrics.current_frame_bytes_retained += current.current_frame_bytes_retained;
            metrics.working_buffer_bytes_retained += current.working_buffer_bytes_retained;
        }
        metrics
    }
}
