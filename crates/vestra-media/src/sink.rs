use vestra_render::CompletedFrame;

use crate::MediaError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SinkResult {
    pub frames_written: u64,
}

/// Consumes completed CPU-accessible frames in output order.
pub trait FrameSink {
    fn write_frame(&mut self, frame: &CompletedFrame) -> Result<(), MediaError>;
    fn finish(&mut self) -> Result<SinkResult, MediaError>;
    /// Stops the sink and releases resources. Calling this more than once is harmless.
    fn abort(&mut self) -> Result<(), MediaError>;
}
