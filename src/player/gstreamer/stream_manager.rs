use anyhow::Result;
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, trace, warn};

/// Information about a stream (audio, subtitle, etc.)
#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub stream_id: String,
    pub stream_type: gst::StreamType,
    pub tags: Option<gst::TagList>,
    pub caps: Option<gst::Caps>,
    pub index: i32,
    pub language: Option<String>,
    pub codec: Option<String>,
}

/// Manages stream collections and track selection for GStreamer player
pub struct StreamManager {
    stream_collection: Arc<Mutex<Option<gst::StreamCollection>>>,
    audio_streams: Arc<Mutex<Vec<StreamInfo>>>,
    subtitle_streams: Arc<Mutex<Vec<StreamInfo>>>,
    current_audio_stream: Arc<Mutex<Option<String>>>,
    current_subtitle_stream: Arc<Mutex<Option<String>>>,
}

impl StreamManager {
    /// Create a new StreamManager
    pub fn new() -> Self {
        Self {
            stream_collection: Arc::new(Mutex::new(None)),
            audio_streams: Arc::new(Mutex::new(Vec::new())),
            subtitle_streams: Arc::new(Mutex::new(Vec::new())),
            current_audio_stream: Arc::new(Mutex::new(None)),
            current_subtitle_stream: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a StreamManager from existing Arc references
    pub fn from_arcs(
        stream_collection: Arc<Mutex<Option<gst::StreamCollection>>>,
        audio_streams: Arc<Mutex<Vec<StreamInfo>>>,
        subtitle_streams: Arc<Mutex<Vec<StreamInfo>>>,
        current_audio_stream: Arc<Mutex<Option<String>>>,
        current_subtitle_stream: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            stream_collection,
            audio_streams,
            subtitle_streams,
            current_audio_stream,
            current_subtitle_stream,
        }
    }

    /// Clear all stream collections (used when loading new media)
    pub fn clear(&self) {
        *self.stream_collection.lock().unwrap() = None;
        self.audio_streams.lock().unwrap().clear();
        self.subtitle_streams.lock().unwrap().clear();
        *self.current_audio_stream.lock().unwrap() = None;
        *self.current_subtitle_stream.lock().unwrap() = None;
        debug!("StreamManager - Cleared all stream collections");
    }

    /// Process a stream collection from GStreamer
    pub fn process_stream_collection_sync(&self, collection: &gst::StreamCollection) {
        debug!("Processing stream collection...");

        // Store the collection
        if let Ok(mut guard) = self.stream_collection.try_lock() {
            *guard = Some(collection.clone());
        }

        // Parse and categorize streams
        let mut audio = Vec::new();
        let mut subtitles = Vec::new();
        let mut audio_index = 0;
        let mut subtitle_index = 0;
        let mut video_count = 0;

        for i in 0..collection.len() {
            let idx = i as u32;
            if let Some(stream) = collection.stream(idx) {
                let stream_id = stream
                    .stream_id()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("stream-{}", idx));
                let stream_type = stream.stream_type();
                let tags = stream.tags();
                let caps = stream.caps();

                // Extract language from tags if available
                let language = tags.as_ref().and_then(|t| {
                    t.index::<gst::tags::LanguageCode>(0)
                        .map(|val| val.get().to_string())
                });

                // Extract codec from caps if available
                let codec = caps
                    .as_ref()
                    .and_then(|c| c.structure(0).map(|s| s.name().to_string()));

                let stream_info = StreamInfo {
                    stream_id: stream_id.clone(),
                    stream_type,
                    tags,
                    caps,
                    index: 0,
                    language: language.clone(),
                    codec: codec.clone(),
                };

                if stream_type.contains(gst::StreamType::VIDEO) {
                    video_count += 1;
                    trace!("Found VIDEO stream: {} (codec: {:?})", stream_id, codec);
                } else if stream_type.contains(gst::StreamType::AUDIO) {
                    let mut info = stream_info;
                    info.index = audio_index;
                    audio_index += 1;
                    trace!(
                        "Found AUDIO stream #{}: {} (language: {:?})",
                        info.index, info.stream_id, info.language
                    );
                    audio.push(info);
                } else if stream_type.contains(gst::StreamType::TEXT) {
                    let mut info = stream_info;
                    info.index = subtitle_index;
                    subtitle_index += 1;
                    trace!(
                        "Found TEXT stream #{}: {} (language: {:?})",
                        info.index, info.stream_id, info.language
                    );
                    subtitles.push(info);
                } else {
                    trace!("Found OTHER stream type: {:?}", stream_type);
                }
            }
        }

        debug!(
            "Stream collection: {} video, {} audio, {} subtitle streams",
            video_count,
            audio.len(),
            subtitles.len()
        );

        // Update stored streams
        if let Ok(mut guard) = self.audio_streams.try_lock() {
            *guard = audio;
        }
        if let Ok(mut guard) = self.subtitle_streams.try_lock() {
            *guard = subtitles;
        }
    }

    /// Send default stream selection event to playbin
    pub fn send_default_stream_selection(
        &self,
        collection: &gst::StreamCollection,
        playbin: &gst::Element,
    ) {
        let mut selected_streams = Vec::new();
        let mut has_video = false;
        let mut has_audio = false;

        debug!("Building default stream selection...");
        for i in 0..collection.len() {
            let idx = i as u32;
            if let Some(stream) = collection.stream(idx) {
                let stream_id = stream
                    .stream_id()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("stream-{}", idx));
                let stream_type = stream.stream_type();

                // Select first stream of each type by default
                if !has_video && stream_type.contains(gst::StreamType::VIDEO) {
                    trace!("Selecting video stream: {}", stream_id);
                    selected_streams.push(stream_id);
                    has_video = true;
                } else if !has_audio && stream_type.contains(gst::StreamType::AUDIO) {
                    trace!("Selecting audio stream: {}", stream_id);
                    selected_streams.push(stream_id);
                    has_audio = true;
                } else if stream_type.contains(gst::StreamType::TEXT) {
                    // Don't select text by default - trace level only
                    trace!(
                        "Skipping text stream: {} (not selected by default)",
                        stream_id
                    );
                }
            }
        }

        if !selected_streams.is_empty() {
            debug!(
                "Sending SELECT_STREAMS event with {} streams",
                selected_streams.len()
            );
            let stream_refs: Vec<&str> = selected_streams.iter().map(|s| s.as_str()).collect();
            let event = gst::event::SelectStreams::new(stream_refs.iter().copied());
            if playbin.send_event(event) {
                debug!("Successfully sent SELECT_STREAMS event");
            } else {
                error!("Failed to send SELECT_STREAMS event");
            }
        } else {
            warn!("No streams selected - this might cause playback issues");
        }
    }

    /// Process streams selected message
    pub fn process_streams_selected(&self, collection: &gst::StreamCollection) {
        debug!("Processing streams selected message");
        debug!("StreamsSelected: {} total streams", collection.len());

        // Clear current selections
        if let Ok(mut audio_guard) = self.current_audio_stream.try_lock() {
            *audio_guard = None;
        }
        if let Ok(mut subtitle_guard) = self.current_subtitle_stream.try_lock() {
            *subtitle_guard = None;
        }

        // Count and categorize streams for summary
        let mut video_count = 0;
        let mut audio_count = 0;
        let mut subtitle_count = 0;
        let mut selected_audio: Option<String> = None;

        for i in 0..collection.len() {
            let idx = i as u32;
            if let Some(stream) = collection.stream(idx) {
                let stream_id = stream
                    .stream_id()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("stream-{}", idx));
                let stream_type = stream.stream_type();

                if stream_type.contains(gst::StreamType::VIDEO) {
                    video_count += 1;
                } else if stream_type.contains(gst::StreamType::AUDIO) {
                    audio_count += 1;
                    // Mark first audio as selected
                    if selected_audio.is_none() {
                        selected_audio = Some(stream_id.clone());
                        if let Ok(mut guard) = self.current_audio_stream.try_lock() {
                            *guard = Some(stream_id.clone());
                        }
                    }
                } else if stream_type.contains(gst::StreamType::TEXT) {
                    subtitle_count += 1;
                }

                trace!("Stream {}: id={}, type={:?}", idx, stream_id, stream_type);
            }
        }

        info!(
            "Stream selection: {} video, {} audio, {} subtitle tracks",
            video_count, audio_count, subtitle_count
        );
        if let Some(audio_id) = &selected_audio {
            debug!("Auto-selected audio stream: {}", audio_id);
        }

        // Store this collection as our stream collection if we don't have one yet
        if self
            .stream_collection
            .lock()
            .map(|guard| guard.is_none())
            .unwrap_or(false)
        {
            debug!("Using StreamsSelected collection as stream collection");
            self.process_stream_collection_sync(collection);
        }
    }

    /// Get available audio tracks
    pub fn get_audio_tracks(&self, playbin: Option<&gst::Element>) -> Vec<(i32, String)> {
        let mut tracks = Vec::new();
        let audio_streams = self.audio_streams.lock().unwrap();

        for stream in audio_streams.iter() {
            let track_name = if let Some(ref lang) = stream.language {
                // Format track name with language
                format!("Audio Track {} ({})", stream.index + 1, lang)
            } else if let Some(ref codec) = stream.codec {
                // Format with codec if no language
                format!("Audio Track {} [{}]", stream.index + 1, codec)
            } else {
                // Default format
                format!("Audio Track {}", stream.index + 1)
            };
            tracks.push((stream.index, track_name));
        }

        // No workaround needed - stream collections are now reliably processed
        // during load_media() before any track queries can happen
        if tracks.is_empty() {
            if let Some(pb) = playbin {
                let timeout = if cfg!(target_os = "macos") {
                    gst::ClockTime::from_mseconds(100)
                } else {
                    gst::ClockTime::ZERO
                };
                let (_, current, _) = pb.state(timeout);

                if current < gst::State::Paused {
                    debug!("Playbin not in PAUSED/PLAYING state yet, no audio tracks available");
                } else {
                    // Stream collection should have been received during preroll
                    warn!(
                        "Playbin is ready but no stream collection available - this indicates a timing issue"
                    );
                }
            }
        }

        tracks
    }

    /// Get available subtitle tracks
    pub fn get_subtitle_tracks(&self) -> Vec<(i32, String)> {
        let mut tracks = Vec::new();

        // Add "None" option first
        tracks.push((-1, "None".to_string()));

        let subtitle_streams = self.subtitle_streams.lock().unwrap();

        for stream in subtitle_streams.iter() {
            let track_name = if let Some(ref lang) = stream.language {
                // Format track name with language
                format!("Subtitle {} ({})", stream.index + 1, lang)
            } else {
                // Default format
                format!("Subtitle {}", stream.index + 1)
            };
            tracks.push((stream.index, track_name));
        }

        tracks
    }

    /// Set audio track by index
    pub fn set_audio_track(&self, track_index: i32, playbin: &gst::Element) -> Result<()> {
        debug!("Selecting audio track: {}", track_index);

        let audio_streams = self.audio_streams.lock().unwrap();
        let subtitle_streams = self.subtitle_streams.lock().unwrap();

        debug!("Available audio streams: {}", audio_streams.len());

        // Find the audio stream with the given index
        let new_audio_stream = audio_streams.iter().find(|s| s.index == track_index);

        if let Some(new_stream) = new_audio_stream {
            debug!(
                "Found audio stream for index {}: {}",
                track_index, new_stream.stream_id
            );

            // Build list of streams to select
            let mut selected_streams = Vec::new();

            // Add the new audio stream
            selected_streams.push(new_stream.stream_id.clone());
            debug!("Adding audio: {}", new_stream.stream_id);

            // Keep the current subtitle stream if one is selected
            if let Some(ref current_sub) = *self.current_subtitle_stream.lock().unwrap()
                && subtitle_streams.iter().any(|s| s.stream_id == *current_sub)
            {
                selected_streams.push(current_sub.clone());
                debug!("Keeping subtitle: {}", current_sub);
            }

            // Also need to include video stream (playbin3 requires all streams)
            // Get the current stream collection to find video streams
            if let Some(ref collection) = *self.stream_collection.lock().unwrap() {
                for i in 0..collection.len() {
                    let idx = i as u32;
                    if let Some(stream) = collection.stream(idx)
                        && stream.stream_type().contains(gst::StreamType::VIDEO)
                    {
                        let stream_id = stream
                            .stream_id()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "video-stream".to_string());
                        selected_streams.push(stream_id.clone());
                        debug!("Adding video: {}", stream_id);
                        break; // Usually only one video stream
                    }
                }
            }

            // Create and send the select-streams event
            debug!(
                "Sending SELECT_STREAMS event with {} streams",
                selected_streams.len()
            );
            let stream_refs: Vec<&str> = selected_streams.iter().map(|s| s.as_str()).collect();
            let event = gst::event::SelectStreams::new(stream_refs.iter().copied());
            if playbin.send_event(event) {
                debug!("SELECT_STREAMS event sent successfully");
                // Update current audio stream
                *self.current_audio_stream.lock().unwrap() = Some(new_stream.stream_id.clone());
                info!(
                    "Selected audio track {}: {}",
                    track_index,
                    new_stream.language.as_deref().unwrap_or("Unknown")
                );
            } else {
                error!("Failed to send SELECT_STREAMS event");
                return Err(anyhow::anyhow!("Failed to select audio track"));
            }
        } else {
            error!(
                "Audio track with index {} not found in {} available tracks",
                track_index,
                audio_streams.len()
            );
            return Err(anyhow::anyhow!("Audio track {} not found", track_index));
        }

        Ok(())
    }

    /// Set subtitle track by index
    pub fn set_subtitle_track(&self, track_index: i32, playbin: &gst::Element) -> Result<()> {
        debug!("Selecting subtitle track: {}", track_index);

        let audio_streams = self.audio_streams.lock().unwrap();
        let subtitle_streams = self.subtitle_streams.lock().unwrap();

        debug!("Available subtitle streams: {}", subtitle_streams.len());

        // Build list of streams to select
        let mut selected_streams = Vec::new();

        // Keep the current audio stream
        if let Some(ref current_audio) = *self.current_audio_stream.lock().unwrap() {
            if audio_streams.iter().any(|s| s.stream_id == *current_audio) {
                selected_streams.push(current_audio.clone());
                debug!("Keeping audio: {}", current_audio);
            }
        } else if let Some(first_audio) = audio_streams.first() {
            // If no current audio, select the first one
            selected_streams.push(first_audio.stream_id.clone());
            debug!("Adding first audio: {}", first_audio.stream_id);
        }

        // Add the subtitle stream if not "None" (-1)
        if track_index >= 0 {
            if let Some(subtitle_stream) = subtitle_streams.iter().find(|s| s.index == track_index)
            {
                debug!("Adding subtitle: {}", subtitle_stream.stream_id);
                selected_streams.push(subtitle_stream.stream_id.clone());

                // Ensure text flag is enabled
                playbin.set_property_from_str(
                    "flags",
                    "soft-colorbalance+deinterlace+soft-volume+audio+video+text",
                );
                debug!("Enabled text flag in playbin");
            } else {
                error!("Subtitle track with index {} not found", track_index);
                return Err(anyhow::anyhow!("Subtitle track {} not found", track_index));
            }
        } else {
            // Disable subtitles by not including any text stream
            debug!("Disabling subtitles (index = -1)");
            *self.current_subtitle_stream.lock().unwrap() = None;

            // Can still keep text flag enabled, just don't select any text stream
            playbin.set_property_from_str(
                "flags",
                "soft-colorbalance+deinterlace+soft-volume+audio+video",
            );
            debug!("Disabled text flag in playbin");
        }

        // Include video stream (required for playbin3)
        if let Some(ref collection) = *self.stream_collection.lock().unwrap() {
            for i in 0..collection.len() {
                let idx = i as u32;
                if let Some(stream) = collection.stream(idx)
                    && stream.stream_type().contains(gst::StreamType::VIDEO)
                {
                    let stream_id = stream
                        .stream_id()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "video-stream".to_string());
                    selected_streams.push(stream_id.clone());
                    debug!("Adding video: {}", stream_id);
                    break;
                }
            }
        }

        // Send the select-streams event if we have streams to select
        if !selected_streams.is_empty() {
            debug!(
                "Sending SELECT_STREAMS event with {} streams",
                selected_streams.len()
            );
            let stream_refs: Vec<&str> = selected_streams.iter().map(|s| s.as_str()).collect();
            let event = gst::event::SelectStreams::new(stream_refs.iter().copied());
            if playbin.send_event(event) {
                debug!("SELECT_STREAMS event sent successfully");
                // Update current subtitle stream
                if track_index >= 0 {
                    if let Some(sub_stream) =
                        subtitle_streams.iter().find(|s| s.index == track_index)
                    {
                        *self.current_subtitle_stream.lock().unwrap() =
                            Some(sub_stream.stream_id.clone());
                        info!(
                            "Selected subtitle track {}: {}",
                            track_index,
                            sub_stream.language.as_deref().unwrap_or("Unknown")
                        );
                    }
                } else {
                    *self.current_subtitle_stream.lock().unwrap() = None;
                    info!("Disabled subtitles");
                }
            } else {
                error!("Failed to send SELECT_STREAMS event");
                return Err(anyhow::anyhow!("Failed to select subtitle track"));
            }
        } else {
            warn!("No streams to select - this shouldn't happen");
        }

        Ok(())
    }

    /// Get current audio track index
    pub fn get_current_audio_track(&self) -> i32 {
        if let Some(ref current_id) = *self.current_audio_stream.lock().unwrap() {
            // Find the index of the current audio stream
            let audio_streams = self.audio_streams.lock().unwrap();
            for stream in audio_streams.iter() {
                if stream.stream_id == *current_id {
                    return stream.index;
                }
            }
        }
        -1
    }

    /// Get current subtitle track index
    pub fn get_current_subtitle_track(&self) -> i32 {
        if let Some(ref current_id) = *self.current_subtitle_stream.lock().unwrap() {
            // Find the index of the current subtitle stream
            let subtitle_streams = self.subtitle_streams.lock().unwrap();
            for stream in subtitle_streams.iter() {
                if stream.stream_id == *current_id {
                    return stream.index;
                }
            }
        }
        -1 // No subtitle selected
    }

    /// Cycle to next subtitle track
    pub fn cycle_subtitle_track(&self, playbin: &gst::Element) -> Result<()> {
        let subtitle_streams = self.subtitle_streams.lock().unwrap();
        let current = self.get_current_subtitle_track();

        let next_track = if subtitle_streams.is_empty() {
            -1 // No subtitles available
        } else if current == -1 {
            // Currently off, go to first subtitle
            0
        } else {
            // Find next track or loop back to "None"
            let next_idx = current + 1;
            if next_idx >= subtitle_streams.len() as i32 {
                -1 // Loop back to "None"
            } else {
                next_idx
            }
        };

        info!("Cycling subtitle track from {} to {}", current, next_track);
        drop(subtitle_streams); // Release lock before calling set_subtitle_track
        self.set_subtitle_track(next_track, playbin)
    }

    /// Cycle to next audio track
    pub fn cycle_audio_track(&self, playbin: &gst::Element) -> Result<()> {
        let audio_streams = self.audio_streams.lock().unwrap();
        if audio_streams.is_empty() {
            return Ok(()); // No audio tracks to cycle
        }

        let current = self.get_current_audio_track();
        let next_track = if current == -1 {
            // No current track (shouldn't happen), select first
            0
        } else {
            // Cycle to next track or loop back to first
            (current + 1) % audio_streams.len() as i32
        };

        info!("Cycling audio track from {} to {}", current, next_track);
        drop(audio_streams); // Release lock before calling set_audio_track
        self.set_audio_track(next_track, playbin)
    }

    /// Get clones of Arc references for use in async contexts
    pub fn get_refs_for_message_handler(
        &self,
    ) -> (
        Arc<Mutex<Option<gst::StreamCollection>>>,
        Arc<Mutex<Vec<StreamInfo>>>,
        Arc<Mutex<Vec<StreamInfo>>>,
        Arc<Mutex<Option<String>>>,
        Arc<Mutex<Option<String>>>,
    ) {
        (
            self.stream_collection.clone(),
            self.audio_streams.clone(),
            self.subtitle_streams.clone(),
            self.current_audio_stream.clone(),
            self.current_subtitle_stream.clone(),
        )
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}
