//! Bus messages emitted by the GStreamer playback pipeline.

use gst::{Stream, StreamCollection};

#[derive(Debug)]
pub enum PipelineBusMsg {
    StreamCollection(StreamCollection),
    StreamsSelected {
        collection: StreamCollection,
        streams: Vec<Stream>,
    },
}
