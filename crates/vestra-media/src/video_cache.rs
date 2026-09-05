//! Bounded decoded-frame cache for one video decoder session.

use std::{collections::BTreeMap, sync::Arc};

#[cfg(test)]
use image::RgbaImage;

use super::{DecodedVideoFrame, MediaError, checked_frame_bytes};

pub(super) struct FrameCache {
    budget: u64,
    bytes: u64,
    clock: u64,
    entries: BTreeMap<i64, CachedFrame>,
}

struct CachedFrame {
    frame: Arc<DecodedVideoFrame>,
    bytes: u64,
    last_used: u64,
    next_pts: Option<i64>,
}

impl FrameCache {
    pub(super) fn new(budget: u64) -> Self {
        Self {
            budget,
            bytes: 0,
            clock: 0,
            entries: BTreeMap::new(),
        }
    }

    pub(super) fn covering_at(
        &mut self,
        pts: i64,
        final_end: Option<i64>,
    ) -> Option<Arc<DecodedVideoFrame>> {
        let key = self
            .entries
            .range(..=pts)
            .next_back()
            .map(|(key, _)| *key)?;
        let entry = self.entries.get_mut(&key)?;
        let covered = entry
            .next_pts
            .map_or_else(|| final_end.is_some_and(|end| pts < end), |next| pts < next);
        if !covered {
            return None;
        }
        self.clock = self.clock.saturating_add(1);
        entry.last_used = self.clock;
        Some(Arc::clone(&entry.frame))
    }

    pub(super) fn insert(&mut self, frame: DecodedVideoFrame) {
        let Ok(bytes) = checked_frame_bytes(frame.width, frame.height)
            .and_then(|bytes| u64::try_from(bytes).map_err(|_| MediaError::VideoFrameByteOverflow))
        else {
            return;
        };
        if bytes > self.budget {
            return;
        }
        self.clock = self.clock.saturating_add(1);
        if let Some(old) = self.entries.remove(&frame.pts.0) {
            self.bytes = self.bytes.saturating_sub(old.bytes);
        }
        while self.bytes.saturating_add(bytes) > self.budget {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(old) = self.entries.remove(&key) {
                self.bytes = self.bytes.saturating_sub(old.bytes);
            }
        }
        if self.bytes.saturating_add(bytes) <= self.budget {
            self.bytes += bytes;
            if let Some((_, previous)) = self.entries.range_mut(..frame.pts.0).next_back()
                && previous.next_pts.is_none()
            {
                previous.next_pts = Some(frame.pts.0);
            }
            self.entries.insert(
                frame.pts.0,
                CachedFrame {
                    frame: Arc::new(frame),
                    bytes,
                    last_used: self.clock,
                    next_pts: None,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_is_bounded_by_rgba_bytes_and_evicts_lru() {
        let mut cache = FrameCache::new(8);
        let image = |value| DecodedVideoFrame {
            pts: super::super::VideoTimestamp(value),
            width: 1,
            height: 1,
            pixels: Arc::new(RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([value as u8, 0, 0, 255]),
            )),
        };
        cache.insert(image(0));
        cache.insert(image(1));
        assert_eq!(cache.entries.len(), 2);
        let _ = cache.covering_at(0, Some(1));
        cache.insert(image(2));
        assert!(cache.entries.contains_key(&0));
        assert!(!cache.entries.contains_key(&1));
        assert!(cache.bytes <= 8);
    }

    #[test]
    fn sparse_cache_only_hits_when_the_entry_proves_its_covering_interval() {
        let mut cache = FrameCache::new(8);
        let image = |value| DecodedVideoFrame {
            pts: super::super::VideoTimestamp(value),
            width: 1,
            height: 1,
            pixels: Arc::new(RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([value as u8, 0, 0, 255]),
            )),
        };
        cache.insert(image(20));
        cache.insert(image(30));
        let _ = cache.covering_at(20, Some(30));
        cache.insert(image(40));
        assert_eq!(cache.covering_at(35, None), None);
        assert_eq!(cache.covering_at(35, Some(50)), None);
        cache.insert(image(50));
        assert_eq!(
            cache.covering_at(45, None).map(|frame| frame.pts.0),
            Some(40)
        );
    }
}
