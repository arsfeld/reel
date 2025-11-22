use crate::player::gstreamer::stream_manager::{StreamInfo, StreamManager};
use crate::player::gstreamer_player::{BufferingState, PlayerState};
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace};

pub fn handle_bus_message_sync(
    msg: &gst::Message,
    state: &Arc<RwLock<PlayerState>>,
    stream_collection: &Arc<Mutex<Option<gst::StreamCollection>>>,
    audio_streams: &Arc<Mutex<Vec<StreamInfo>>>,
    subtitle_streams: &Arc<Mutex<Vec<StreamInfo>>>,
    current_audio: &Arc<Mutex<Option<String>>>,
    current_subtitle: &Arc<Mutex<Option<String>>>,
    pipeline_ready: &Arc<Mutex<bool>>,
    playbin: &Arc<Mutex<Option<gst::Element>>>,
    buffering_state: &Arc<RwLock<BufferingState>>,
) {
    use gst::MessageView;

    // Log all messages for debugging
    let msg_type = msg.type_();
    if !matches!(msg_type, gst::MessageType::Qos | gst::MessageType::Progress) {
        let src_name = msg
            .src()
            .map(|s| s.name().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Highlight specific message types we're interested in
        if matches!(msg_type, gst::MessageType::StreamCollection) {
            debug!("Stream collection message from {}", src_name);
        } else if matches!(msg_type, gst::MessageType::StreamsSelected) {
            debug!("Streams selected message from {}", src_name);
        } else if matches!(msg_type, gst::MessageType::StreamStart) {
            debug!("Stream start message from {}", src_name);
        } else {
            trace!("Bus message: {:?} from {}", msg_type, src_name);
        }
    }

    match msg.view() {
        MessageView::Eos(_) => {
            info!("GStreamerPlayer - Bus message: End of stream");
            if let Ok(mut state_guard) = state.try_write() {
                *state_guard = PlayerState::Stopped;
            }
        }
        MessageView::Error(err) => {
            error!(
                "GStreamerPlayer - Bus error from {:?}: {} ({:?})",
                err.src().map(|s| s.path_string()),
                err.error(),
                err.debug()
            );
            if let Ok(mut state_guard) = state.try_write() {
                *state_guard = PlayerState::Error;
            }
        }
        MessageView::StateChanged(state_changed) => {
            // Only handle state changes from the playbin element itself
            if let Some(src) = state_changed.src() {
                let element_name = src.name();
                if element_name.starts_with("playbin") {
                    let new_state = state_changed.current();
                    let old_state = state_changed.old();

                    debug!(
                        "GStreamerPlayer - Playbin state changed from {:?} to {:?}",
                        old_state, new_state
                    );

                    // Update internal state based on pipeline state
                    if let Ok(mut state_guard) = state.try_write() {
                        match new_state {
                            gst::State::Playing => {
                                *state_guard = PlayerState::Playing;
                                debug!("State updated to Playing");
                            }
                            gst::State::Paused => {
                                // Only set to Paused if we're not in Loading state
                                // (Loading state transitions through Paused)
                                if !matches!(*state_guard, PlayerState::Loading) {
                                    *state_guard = PlayerState::Paused;
                                    debug!("State updated to Paused");
                                }
                            }
                            gst::State::Ready | gst::State::Null => {
                                // Only update to Stopped if we're not in Error or Loading state
                                if !matches!(
                                    *state_guard,
                                    PlayerState::Error | PlayerState::Loading
                                ) {
                                    *state_guard = PlayerState::Stopped;
                                    debug!("State updated to Stopped");
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        MessageView::Buffering(buffering) => {
            let percent = buffering.percent();
            debug!("GStreamerPlayer - Buffering: {}%", percent);

            // Update buffering state
            if let Ok(mut buffering_guard) = buffering_state.try_write() {
                buffering_guard.percentage = percent;
                buffering_guard.is_buffering = percent < 100;

                if buffering_guard.is_buffering {
                    debug!("Buffering started/ongoing: {}%", percent);
                } else {
                    debug!("Buffering complete: 100%");
                }
            }
        }
        MessageView::AsyncDone(_) => {
            info!("GStreamerPlayer - AsyncDone: Pipeline ready, dimensions should be available");

            // Mark pipeline as ready for seeking
            *pipeline_ready.lock().unwrap() = true;
            info!("GStreamerPlayer - Pipeline marked as ready for seeking");

            // Check if we have received a stream collection yet
            let has_collection = stream_collection
                .lock()
                .map(|guard| guard.is_some())
                .unwrap_or(false);

            if !has_collection {
                info!(
                    "⚠️  AsyncDone but no StreamCollection received yet - this might indicate an issue"
                );

                // Try to manually query for stream information
                if let Ok(Some(pb)) = playbin.lock().map(|p| p.as_ref().cloned()) {
                    info!("Attempting to manually query stream information from playbin3...");

                    // Check if there are any stream-related signals we can query
                    let props = pb.list_properties();
                    for prop in props {
                        let name = prop.name();
                        if name.starts_with("n-")
                            && (name.contains("audio")
                                || name.contains("video")
                                || name.contains("text"))
                        {
                            let value = pb.property_value(name);
                            info!("  {}: {:?}", name, value);
                        }
                    }
                }
            } else {
                debug!("StreamCollection already received");
            }
        }
        MessageView::StreamCollection(collection_msg) => {
            let collection = collection_msg.stream_collection();
            debug!(
                "Stream collection received with {} streams",
                collection.len()
            );

            // Create a temporary StreamManager to process the collection
            let temp_stream_manager = StreamManager::from_arcs(
                stream_collection.clone(),
                audio_streams.clone(),
                subtitle_streams.clone(),
                current_audio.clone(),
                current_subtitle.clone(),
            );

            // Process the collection synchronously
            temp_stream_manager.process_stream_collection_sync(&collection);

            // Send default stream selection
            if let Ok(Some(pb)) = playbin.lock().map(|p| p.as_ref().cloned()) {
                temp_stream_manager.send_default_stream_selection(&collection, &pb);
            }
        }
        MessageView::StreamsSelected(selected_msg) => {
            // Get the collection from the message
            let collection = selected_msg.stream_collection();

            // Create a temporary StreamManager to process the selection
            let temp_stream_manager = StreamManager::from_arcs(
                stream_collection.clone(),
                audio_streams.clone(),
                subtitle_streams.clone(),
                current_audio.clone(),
                current_subtitle.clone(),
            );

            // Process the streams selected message
            temp_stream_manager.process_streams_selected(&collection);
        }
        MessageView::Tag(tag_msg) => {
            // Tags might contain stream information - only log at trace level
            let tags = tag_msg.tags();
            // Only log language tags at trace level since they're very verbose
            if let Some(lang) = tags.index::<gst::tags::LanguageCode>(0) {
                trace!("Found language tag: {}", lang.get());
            }
        }
        MessageView::StreamStart(_stream_start_msg) => {
            debug!("Stream started - collection should follow soon");

            // StreamStart message doesn't provide direct stream access
            // We'll rely on the StreamCollection message that follows
            info!("  StreamStart received - waiting for StreamCollection");

            // Check if this is from a decodebin - they should emit collections
            if let Some(src) = msg.src() {
                let src_name = src.name();
                info!("  StreamStart source: {}", src_name);
                if src_name.to_string().contains("decodebin") {
                    info!(
                        "  ⚠️  StreamStart from decodebin - StreamCollection should have been sent!"
                    );
                }
            }
        }
        MessageView::Element(elem_msg) => {
            // Some elements might send custom messages about streams
            if let Some(s) = elem_msg.structure() {
                let name = s.name();
                if name.contains("stream") || name.contains("collection") || name.contains("select")
                {
                    info!("Element message: {}", name);
                }
            }
        }
        _ => {
            // Log other potentially relevant unhandled messages
            let msg_type = msg.type_();
            if matches!(
                msg_type,
                gst::MessageType::SegmentStart
                    | gst::MessageType::SegmentDone
                    | gst::MessageType::DurationChanged
                    | gst::MessageType::Latency
                    | gst::MessageType::Toc
                    | gst::MessageType::StreamStatus
            ) {
                trace!("Unhandled message: {:?}", msg_type);
            }
        }
    }
}
