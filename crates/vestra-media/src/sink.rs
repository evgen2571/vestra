use crate::MediaError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SinkResult {
    pub frames_written: u64,
}

/// Writes completed RGBA frames in output order, borrowing bytes for each write.
pub trait FrameSink {
    fn write_frame(&mut self, frame_number: u64, rgba: &[u8]) -> Result<(), MediaError>;
    fn finish(&mut self) -> Result<SinkResult, MediaError>;
    /// Stops the sink and releases resources. Calling this more than once is harmless.
    fn abort(&mut self) -> Result<(), MediaError>;
}
