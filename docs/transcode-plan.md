# Plex Transcode and Quality Selection Integration Plan

**Version**: 2.0
**Created**: 2025-10-03
**Updated**: 2025-10-03 (Added Adaptive Quality & Smart Recovery)
**Related Tasks**: task-367 (research), task-209 (implementation)

## Overview

This document outlines the integration of Plex's transcoding/decision endpoint with quality selection UI, adaptive quality management, and the existing chunk-based file cache system. The goal is to enable remote playback, intelligent quality switching based on network conditions, and seamless integration with our progressive download architecture.

## Problem Statement

### Current Issues

1. **Remote Playback Fails**: Direct file URLs only work on local network connections
2. **No Quality Selection**: Users cannot choose different quality/bitrate options
3. **Cache Integration**: Current cache system doesn't account for transcoded streams
4. **Connection Type Awareness**: System doesn't adapt behavior based on local vs remote connection

### Requirements

1. Support remote Plex connections via transcode/decision endpoints
2. Provide UI for quality/resolution selection
3. Integrate transcoded streams with chunk-based cache system
4. Maintain fast local playback with direct URLs
5. Handle quality switching during playback
6. Cache transcoded streams separately from original quality
7. **Automatically adjust quality based on network conditions and playback health**
8. **Detect and recover from playback failures with lower quality**
9. **Monitor bandwidth continuously and adapt progressively**
10. **Provide user control over adaptive quality behavior (Auto/Manual modes)**

## Architecture Overview

### Component Integration

```
┌─────────────────────────────────────────────────────────────────┐
│                     User Interface (Relm4)                      │
│  - Quality selector dropdown                                    │
│  - Shows: Original, 1080p, 720p, 480p, 360p                     │
│  - Displays current quality and bitrate                         │
└─────────────────────┬───────────────────────────────────────────┘
                      │ Quality selection event
                      ↓
┌─────────────────────────────────────────────────────────────────┐
│                    PlayerPage / PlayerController                │
│  - Receives quality change request                              │
│  - Calls MediaService with new quality                          │
│  - Updates player with new stream URL                           │
└─────────────────────┬───────────────────────────────────────────┘
                      │ get_stream_with_quality()
                      ↓
┌─────────────────────────────────────────────────────────────────┐
│                         MediaService                            │
│  - Fetches stream URL from backend                              │
│  - Determines if cache should be used                           │
│  - Coordinates backend + cache layer                            │
└────────┬────────────────────────────┬─────────────────────────┘
         │                            │
         ↓ (remote)                   ↓ (with cache)
┌────────────────────┐     ┌──────────────────────────────────────┐
│   PlexBackend      │     │        FileCache                     │
│  - Connection type │     │  - get_cached_stream()               │
│  - Decision API    │────→│  - Quality-aware cache keys          │
│  - Quality options │     │  - Chunk-based downloads             │
└────────────────────┘     └──────────┬───────────────────────────┘
                                      │
                                      ↓
                          ┌────────────────────────┐
                          │     CacheProxy         │
                          │  - Serves via HTTP     │
                          │  - Progressive stream  │
                          └────────────────────────┘
```

## Core Components

### 1. PlexBackend Enhancements

#### Connection Type Detection

**Implementation**: Use existing `ConnectionService` infrastructure

The application already has a comprehensive connection management system in `src/services/core/connection.rs` with:
- Global `ConnectionCache` that stores connection states
- `ConnectionService::select_best_connection()` that tests and caches connections
- `ConnectionState` that tracks connection type (Local, Remote, Relay)
- Database persistence of connection quality

**Integration with PlexBackend**: `src/backends/plex/mod.rs`

Add a reference to the source ID and query the connection cache:

```rust
use crate::services::core::connection::ConnectionService;
use crate::services::core::connection_cache::ConnectionType;

impl PlexBackend {
    /// Get the current connection type from ConnectionService cache
    pub async fn is_local_connection(&self) -> bool {
        let cache = ConnectionService::cache();
        if let Some(state) = cache.get(&self.source_id).await {
            state.is_local()
        } else {
            // If no cached connection, assume remote (safer default)
            false
        }
    }

    /// Get connection location string for decision endpoint
    pub async fn get_connection_location(&self) -> &str {
        if self.is_local_connection().await {
            "lan"
        } else {
            "wan"
        }
    }
}
```

**Note**: The `PlexBackend` already has access to its `source_id` field, which is used as the key in the `ConnectionCache`. The `ConnectionService::select_best_connection()` is called during backend initialization and periodically by the connection monitor, so the cache is kept up-to-date automatically.

#### Decision Endpoint Implementation

**New file**: `src/backends/plex/api/decision.rs`

```rust
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use super::client::PlexApi;
use crate::models::{QualityOption, Resolution, StreamInfo};

#[derive(Debug, Deserialize)]
struct DecisionResponse {
    #[serde(rename = "MediaContainer")]
    media_container: DecisionMediaContainer,
}

#[derive(Debug, Deserialize)]
struct DecisionMediaContainer {
    #[serde(default)]
    #[serde(rename = "Metadata")]
    metadata: Vec<DecisionMetadata>,
}

#[derive(Debug, Deserialize)]
struct DecisionMetadata {
    #[serde(rename = "Media", default)]
    media: Vec<DecisionMedia>,
}

#[derive(Debug, Deserialize)]
struct DecisionMedia {
    #[serde(rename = "Part", default)]
    parts: Vec<DecisionPart>,

    #[serde(rename = "videoDecision")]
    video_decision: Option<String>,

    #[serde(rename = "audioDecision")]
    audio_decision: Option<String>,

    #[serde(rename = "protocol")]
    protocol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DecisionPart {
    #[serde(rename = "key")]
    key: String,

    #[serde(rename = "decision")]
    decision: Option<String>,
}

impl PlexApi {
    /// Get stream URL via decision endpoint (for remote connections)
    pub async fn get_stream_url_via_decision(
        &self,
        media_id: &str,
        direct_play: bool,
        max_bitrate_kbps: Option<u64>,
        resolution: Option<Resolution>,
        is_local: bool,
    ) -> Result<StreamInfo> {
        let path = format!("/library/metadata/{}", media_id);

        // Build query parameters
        let mut params = vec![
            ("path", path.as_str()),
            ("mediaIndex", "0"),
            ("partIndex", "0"),
            ("protocol", "http"),
            ("hasMDE", "1"),
            ("location", if is_local { "lan" } else { "wan" }),
        ];

        // Direct play settings
        let direct_play_str = if direct_play { "1" } else { "0" };
        params.push(("directPlay", direct_play_str));
        params.push(("directStream", direct_play_str));

        // Quality parameters (for transcoding)
        let bitrate_str;
        let resolution_str;
        if !direct_play {
            if let Some(bitrate) = max_bitrate_kbps {
                bitrate_str = bitrate.to_string();
                params.push(("maxVideoBitrate", &bitrate_str));
            }

            if let Some(res) = resolution {
                resolution_str = format!("{}x{}", res.width, res.height);
                params.push(("videoResolution", &resolution_str));
            }

            params.push(("fastSeek", "1"));
        }

        // Make decision request
        let url = self.build_url("/video/:/transcode/universal/decision");

        let response = self
            .client
            .get(&url)
            .query(&params)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Decision endpoint failed: {}", response.status()));
        }

        let decision_response: DecisionResponse = response.json().await?;

        // Extract stream URL from decision response
        if let Some(metadata) = decision_response.media_container.metadata.first()
            && let Some(media) = metadata.media.first()
            && let Some(part) = media.parts.first()
        {
            // Construct the full stream URL
            let stream_url = if part.key.starts_with("http://") || part.key.starts_with("https://") {
                part.key.clone()
            } else {
                let separator = if part.key.contains('?') { "&" } else { "?" };
                format!(
                    "{}{}{}X-Plex-Token={}",
                    self.base_url, part.key, separator, self.auth_token
                )
            };

            // Build StreamInfo from decision
            Ok(StreamInfo {
                url: stream_url,
                direct_play,
                video_codec: String::new(), // Would need to fetch from metadata
                audio_codec: String::new(),
                container: String::new(),
                bitrate: max_bitrate_kbps.unwrap_or(0) * 1000,
                resolution: resolution.unwrap_or(Resolution { width: 1920, height: 1080 }),
                quality_options: Vec::new(), // Populated by get_stream_url
            })
        } else {
            Err(anyhow!("Invalid decision response structure"))
        }
    }
}
```

#### Enhanced get_stream_url

**Update**: `src/backends/plex/api/streaming.rs`

```rust
impl PlexApi {
    pub async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo> {
        // First, fetch metadata to build quality options
        let url = self.build_url(&format!("/library/metadata/{}", media_id));

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get media info: {}", response.status()));
        }

        let response_text = response.text().await?;
        let plex_response: PlexMediaResponse = serde_json::from_str(&response_text)?;

        if let Some(metadata) = plex_response.media_container.metadata.first()
            && let Some(media) = metadata.media.first()
            && let Some(part) = media.parts.as_ref().and_then(|p| p.first())
        {
            // Determine connection type from PlexBackend
            // This requires passing connection info or checking in backend

            // For now, construct direct play URL for local connections
            let direct_play_url = if part.key.starts_with("http://") || part.key.starts_with("https://") {
                part.key.clone()
            } else {
                let separator = if part.key.contains('?') { "&" } else { "?" };
                format!(
                    "{}{}{}X-Plex-Token={}",
                    self.base_url, part.key, separator, self.auth_token
                )
            };

            // Build quality options
            let original_bitrate = media.bitrate.unwrap_or(0);
            let original_width = media.width.unwrap_or(1920);
            let original_height = media.height.unwrap_or(1080);

            let mut quality_options = Vec::new();

            // Original quality (direct play)
            quality_options.push(QualityOption {
                name: format!("Original ({}p)", original_height),
                resolution: Resolution {
                    width: original_width,
                    height: original_height,
                },
                bitrate: original_bitrate,
                url: direct_play_url.clone(),
                requires_transcode: false,
            });

            // Transcode quality options
            let transcode_qualities = vec![
                ("1080p", 1920, 1080, 8000),
                ("720p", 1280, 720, 4000),
                ("480p", 854, 480, 2000),
                ("360p", 640, 360, 1000),
            ];

            for (name, width, height, bitrate_kbps) in transcode_qualities {
                if height < original_height {
                    // Note: Actual URL will be generated via decision endpoint when selected
                    quality_options.push(QualityOption {
                        name: name.to_string(),
                        resolution: Resolution { width, height },
                        bitrate: (bitrate_kbps * 1000) as u64,
                        url: String::new(), // Placeholder - generated on selection
                        requires_transcode: true,
                    });
                }
            }

            Ok(StreamInfo {
                url: direct_play_url,
                direct_play: true,
                video_codec: media.video_codec.clone().unwrap_or_default(),
                audio_codec: media.audio_codec.clone().unwrap_or_default(),
                container: part.container.clone().unwrap_or_default(),
                bitrate: original_bitrate,
                resolution: Resolution {
                    width: original_width,
                    height: original_height,
                },
                quality_options,
            })
        } else {
            Err(anyhow!("Failed to get stream info for media {}", media_id))
        }
    }

    /// Get stream URL for specific quality
    pub async fn get_stream_url_for_quality(
        &self,
        media_id: &str,
        quality: &QualityOption,
        is_local: bool,
    ) -> Result<String> {
        if quality.requires_transcode {
            // Use decision endpoint for transcoded streams
            let stream_info = self.get_stream_url_via_decision(
                media_id,
                false, // not direct play
                Some(quality.bitrate / 1000), // convert to kbps
                Some(quality.resolution.clone()),
                is_local,
            ).await?;

            Ok(stream_info.url)
        } else {
            // Use direct URL for original quality
            Ok(quality.url.clone())
        }
    }
}
```

### 2. FileCache Integration

#### Quality-Aware Cache Keys

**Update**: `src/cache/file_cache.rs`

The cache system needs to store different qualities separately:

```rust
impl FileCache {
    /// Get cache entry ID with quality
    async fn get_or_create_entry(
        &self,
        source_id: &str,
        media_id: &str,
        quality: &str, // e.g., "original", "1080p", "720p"
        original_url: &str,
    ) -> Result<i32> {
        // Check existing cache entry with quality
        let cache_repo = CacheRepository::new(self.db.clone());

        if let Some(entry) = cache_repo
            .get_entry_by_key(source_id, media_id, quality)
            .await?
        {
            return Ok(entry.id);
        }

        // Create new cache entry for this quality
        cache_repo
            .create_entry(source_id, media_id, quality, original_url)
            .await
    }

    /// Get cached stream with quality selection
    pub async fn get_cached_stream_with_quality(
        &self,
        source_id: &str,
        media_id: &str,
        quality: &str,
        original_url: &str,
    ) -> Result<String> {
        let entry_id = self
            .get_or_create_entry(source_id, media_id, quality, original_url)
            .await?;

        // Register with proxy
        let proxy_url = self.proxy.register_stream(entry_id, original_url).await?;

        // Start background download for this quality
        self.chunk_manager
            .request_sequential_download(entry_id, original_url, Priority::LOW)
            .await;

        Ok(proxy_url)
    }
}
```

#### Cache Repository Schema

**Already exists**: The `cache_entries` table has a `quality` field:

```sql
CREATE TABLE cache_entries (
    id INTEGER PRIMARY KEY,
    source_id TEXT NOT NULL,
    media_id TEXT NOT NULL,
    quality TEXT NOT NULL,
    -- ...
    UNIQUE(source_id, media_id, quality)
);
```

This already supports quality-based caching! ✅

### 3. Quality Selection UI

#### UI Component Structure

**New component**: `src/ui/shared/quality_selector.rs`

```rust
use relm4::prelude::*;
use crate::models::{QualityOption, StreamInfo};

#[derive(Debug)]
pub struct QualitySelector {
    qualities: Vec<QualityOption>,
    selected_index: usize,
}

#[derive(Debug)]
pub enum QualitySelectorMsg {
    UpdateQualities(Vec<QualityOption>),
    SelectQuality(usize),
}

#[derive(Debug)]
pub enum QualitySelectorOutput {
    QualityChanged(QualityOption),
}

#[relm4::component(pub)]
impl Component for QualitySelector {
    type Init = Vec<QualityOption>;
    type Input = QualitySelectorMsg;
    type Output = QualitySelectorOutput;
    type CommandOutput = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 8,

            gtk::Label {
                set_label: "Quality:",
                set_halign: gtk::Align::Start,
            },

            gtk::DropDown {
                set_model: Some(&model.quality_model()),
                set_selected: model.selected_index as u32,

                connect_selected_notify[sender] => move |dropdown| {
                    let selected = dropdown.selected() as usize;
                    sender.input(QualitySelectorMsg::SelectQuality(selected));
                },
            },

            gtk::Label {
                #[watch]
                set_label: &model.current_quality_info(),
                add_css_class: "caption",
            },
        }
    }

    fn init(
        qualities: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = QualitySelector {
            qualities,
            selected_index: 0,
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            QualitySelectorMsg::UpdateQualities(qualities) => {
                self.qualities = qualities;
                self.selected_index = 0;
            }
            QualitySelectorMsg::SelectQuality(index) => {
                if index < self.qualities.len() {
                    self.selected_index = index;
                    let quality = self.qualities[index].clone();
                    sender.output(QualitySelectorOutput::QualityChanged(quality));
                }
            }
        }
    }
}

impl QualitySelector {
    fn quality_model(&self) -> gtk::StringList {
        let list = gtk::StringList::new(&[]);
        for quality in &self.qualities {
            list.append(&quality.name);
        }
        list
    }

    fn current_quality_info(&self) -> String {
        if let Some(quality) = self.qualities.get(self.selected_index) {
            format!(
                "{}x{} @ {} Mbps",
                quality.resolution.width,
                quality.resolution.height,
                quality.bitrate / 1_000_000
            )
        } else {
            String::new()
        }
    }
}
```

#### Integration in PlayerPage

**Update**: `src/ui/pages/player.rs`

Add quality selector to player controls:

```rust
pub struct PlayerPage {
    // ... existing fields
    quality_selector: Controller<QualitySelector>,
    current_stream_info: Option<StreamInfo>,
}

#[derive(Debug)]
pub enum PlayerPageMsg {
    // ... existing messages
    StreamInfoLoaded(StreamInfo),
    QualityChanged(QualityOption),
}

impl AsyncComponent for PlayerPage {
    fn init_model(init: Self::Init) -> Self {
        // ... existing init

        let quality_selector = QualitySelector::builder()
            .launch(Vec::new())
            .forward(sender.input_sender(), |msg| match msg {
                QualitySelectorOutput::QualityChanged(quality) => {
                    PlayerPageMsg::QualityChanged(quality)
                }
            });

        Self {
            // ... existing fields
            quality_selector,
            current_stream_info: None,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: AsyncComponentSender<Self>) {
        match msg {
            PlayerPageMsg::StreamInfoLoaded(stream_info) => {
                // Update quality selector with available options
                self.quality_selector.emit(
                    QualitySelectorMsg::UpdateQualities(stream_info.quality_options.clone())
                );
                self.current_stream_info = Some(stream_info);
            }

            PlayerPageMsg::QualityChanged(quality) => {
                // Switch to new quality
                sender.oneshot_command(async move {
                    // Request new stream URL for selected quality
                    // This will be implemented in MediaService
                });
            }

            // ... existing messages
        }
    }

    view! {
        gtk::Box {
            // ... existing player view

            // Add quality selector to controls overlay
            gtk::Overlay {
                // ... video widget

                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_valign: gtk::Align::End,

                    // Player controls
                    gtk::Box {
                        // ... existing controls

                        // Quality selector
                        model.quality_selector.widget(),
                    }
                }
            }
        }
    }
}
```

### 4. MediaService Enhancement

**Update**: `src/services/core/media.rs`

Add quality-aware stream fetching:

```rust
use crate::services::core::connection::ConnectionService;

impl MediaService {
    /// Get stream with specific quality
    pub async fn get_stream_with_quality(
        &self,
        source_id: &SourceId,
        media_id: &MediaItemId,
        quality: &QualityOption,
    ) -> Result<String> {
        // Get backend for source
        let backend = self.backend_service.get_backend(source_id).await?;

        // Check connection type from ConnectionService cache
        let cache = ConnectionService::cache();
        let is_local = if let Some(state) = cache.get(source_id).await {
            state.is_local()
        } else {
            false // Default to remote if no cached connection
        };

        // Get stream URL for quality
        let stream_url = if let Some(plex_backend) = backend.as_any().downcast_ref::<PlexBackend>() {
            let api = plex_backend.get_api_for_playqueue().await
                .ok_or_else(|| anyhow!("API not available"))?;

            api.get_stream_url_for_quality(media_id.as_ref(), quality, is_local).await?
        } else {
            quality.url.clone()
        };

        // Determine cache key based on quality
        let quality_key = if quality.requires_transcode {
            &quality.name // "1080p", "720p", etc.
        } else {
            "original"
        };

        // Get cached stream
        let cached_url = self.file_cache
            .get_cached_stream_with_quality(
                source_id.as_ref(),
                media_id.as_ref(),
                quality_key,
                &stream_url,
            )
            .await?;

        Ok(cached_url)
    }
}
```

## Adaptive Quality System

### Overview

The adaptive quality system automatically adjusts video quality based on network conditions and playback performance. This ensures smooth playback even when bandwidth fluctuates or the initial quality selection was too high for the connection.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Player (GStreamer/MPV)                     │
│  - Emits state changes (playing, buffering, error)              │
│  - Reports buffer levels                                        │
└────────────────────┬────────────────────────────────────────────┘
                     │ State events
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│                      PlaybackMonitor                            │
│  - Tracks player state transitions                              │
│  - Detects buffering events and duration                        │
│  - Identifies playback stalls and failures                      │
│  - Measures time between buffers                                │
└────────────────────┬────────────────────────────────────────────┘
                     │ Health metrics
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│                    BandwidthMonitor                             │
│  - Monitors chunk download speeds from CacheProxy               │
│  - Calculates moving average bandwidth (30s window)             │
│  - Tracks bandwidth trend (increasing/decreasing/stable)        │
│  - Estimates available bandwidth vs required bandwidth          │
└────────────────────┬────────────────────────────────────────────┘
                     │ Bandwidth metrics
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│                 AdaptiveQualityManager                          │
│  - Receives metrics from both monitors                          │
│  - Applies quality adjustment algorithm                         │
│  - Triggers quality changes automatically                       │
│  - Respects user preferences (auto/manual mode)                 │
│  - Implements cooldown periods between changes                  │
└────────────────────┬────────────────────────────────────────────┘
                     │ Quality change request
                     ↓
┌─────────────────────────────────────────────────────────────────┐
│                    PlayerController                             │
│  - Executes quality change                                      │
│  - Updates UI to show adaptive quality active                   │
│  - Logs quality change reasoning                                │
└─────────────────────────────────────────────────────────────────┘
```

### Component Details

#### 1. PlaybackMonitor

**Purpose**: Monitor player health and detect issues requiring quality adjustment.

**Implementation**: `src/player/playback_monitor.rs`

```rust
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum PlaybackHealth {
    Healthy,           // Playing smoothly, no issues
    Buffering,         // Currently buffering
    Unstable,          // Frequent buffering (>3 in 60s)
    Failed,            // Playback failed completely
}

#[derive(Debug, Clone)]
pub struct PlaybackMetrics {
    pub health: PlaybackHealth,
    pub buffer_count: u32,           // Buffers in last 60s
    pub average_buffer_duration: Duration,
    pub time_since_last_buffer: Duration,
    pub playback_errors: u32,
}

pub struct PlaybackMonitor {
    state_rx: mpsc::Receiver<PlayerState>,
    metrics: PlaybackMetrics,
    buffer_history: Vec<BufferEvent>,
    metrics_tx: mpsc::Sender<PlaybackMetrics>,
}

#[derive(Debug)]
struct BufferEvent {
    timestamp: Instant,
    duration: Duration,
}

impl PlaybackMonitor {
    pub fn new(
        state_rx: mpsc::Receiver<PlayerState>,
        metrics_tx: mpsc::Sender<PlaybackMetrics>,
    ) -> Self {
        Self {
            state_rx,
            metrics: PlaybackMetrics::default(),
            buffer_history: Vec::new(),
            metrics_tx,
        }
    }

    pub async fn run(&mut self) {
        let mut current_state = PlayerState::Stopped;
        let mut buffer_start: Option<Instant> = None;

        while let Some(state) = self.state_rx.recv().await {
            match (&current_state, &state) {
                // Buffering started
                (PlayerState::Playing, PlayerState::Buffering) => {
                    buffer_start = Some(Instant::now());
                    tracing::warn!("Playback buffering started");
                }

                // Buffering ended
                (PlayerState::Buffering, PlayerState::Playing) => {
                    if let Some(start) = buffer_start {
                        let duration = start.elapsed();
                        self.record_buffer_event(duration);
                        tracing::info!("Buffering ended, duration: {:?}", duration);
                        buffer_start = None;
                    }
                }

                // Playback failed
                (_, PlayerState::Error) => {
                    self.metrics.playback_errors += 1;
                    self.metrics.health = PlaybackHealth::Failed;
                    tracing::error!("Playback error detected");
                }

                _ => {}
            }

            current_state = state;
            self.update_health();

            // Send updated metrics
            let _ = self.metrics_tx.send(self.metrics.clone()).await;
        }
    }

    fn record_buffer_event(&mut self, duration: Duration) {
        let now = Instant::now();

        // Add new buffer event
        self.buffer_history.push(BufferEvent {
            timestamp: now,
            duration,
        });

        // Remove events older than 60s
        self.buffer_history.retain(|event| {
            now.duration_since(event.timestamp) < Duration::from_secs(60)
        });

        // Update metrics
        self.metrics.buffer_count = self.buffer_history.len() as u32;

        if !self.buffer_history.is_empty() {
            let total_duration: Duration = self.buffer_history
                .iter()
                .map(|e| e.duration)
                .sum();

            self.metrics.average_buffer_duration =
                total_duration / self.buffer_history.len() as u32;

            self.metrics.time_since_last_buffer = now
                .duration_since(self.buffer_history.last().unwrap().timestamp);
        }
    }

    fn update_health(&mut self) {
        // Determine health based on buffer frequency
        self.metrics.health = if self.metrics.playback_errors > 0 {
            PlaybackHealth::Failed
        } else if self.metrics.buffer_count >= 3 {
            PlaybackHealth::Unstable
        } else if self.metrics.buffer_count > 0 {
            PlaybackHealth::Buffering
        } else {
            PlaybackHealth::Healthy
        };
    }
}
```

#### 2. BandwidthMonitor

**Purpose**: Track actual download speeds and estimate available bandwidth.

**Implementation**: `src/player/bandwidth_monitor.rs`

```rust
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct BandwidthMetrics {
    pub current_speed_bps: u64,      // Current download speed (bytes/sec)
    pub average_speed_bps: u64,       // Moving average (30s)
    pub trend: BandwidthTrend,
    pub estimated_available_bps: u64, // Conservative estimate
}

#[derive(Debug, Clone, PartialEq)]
pub enum BandwidthTrend {
    Increasing,
    Stable,
    Decreasing,
}

pub struct BandwidthMonitor {
    measurements: VecDeque<SpeedMeasurement>,
    metrics: BandwidthMetrics,
    metrics_tx: mpsc::Sender<BandwidthMetrics>,
}

#[derive(Debug, Clone)]
struct SpeedMeasurement {
    timestamp: Instant,
    bytes_per_second: u64,
}

impl BandwidthMonitor {
    pub fn new(metrics_tx: mpsc::Sender<BandwidthMetrics>) -> Self {
        Self {
            measurements: VecDeque::new(),
            metrics: BandwidthMetrics {
                current_speed_bps: 0,
                average_speed_bps: 0,
                trend: BandwidthTrend::Stable,
                estimated_available_bps: 0,
            },
            metrics_tx,
        }
    }

    /// Record a chunk download
    pub async fn record_download(&mut self, bytes: u64, duration: Duration) {
        let bytes_per_second = if duration.as_secs() > 0 {
            bytes / duration.as_secs()
        } else {
            bytes * 1000 / duration.as_millis().max(1) as u64
        };

        let now = Instant::now();

        // Add measurement
        self.measurements.push_back(SpeedMeasurement {
            timestamp: now,
            bytes_per_second,
        });

        // Keep only last 30 seconds
        while let Some(first) = self.measurements.front() {
            if now.duration_since(first.timestamp) > Duration::from_secs(30) {
                self.measurements.pop_front();
            } else {
                break;
            }
        }

        // Update metrics
        self.update_metrics();

        // Send update
        let _ = self.metrics_tx.send(self.metrics.clone()).await;
    }

    fn update_metrics(&mut self) {
        if self.measurements.is_empty() {
            return;
        }

        // Current speed is most recent measurement
        self.metrics.current_speed_bps = self.measurements
            .back()
            .map(|m| m.bytes_per_second)
            .unwrap_or(0);

        // Average speed over window
        let total: u64 = self.measurements
            .iter()
            .map(|m| m.bytes_per_second)
            .sum();

        self.metrics.average_speed_bps = total / self.measurements.len() as u64;

        // Estimate available bandwidth (conservative: 80% of average)
        self.metrics.estimated_available_bps = (self.metrics.average_speed_bps * 80) / 100;

        // Determine trend
        self.metrics.trend = self.calculate_trend();
    }

    fn calculate_trend(&self) -> BandwidthTrend {
        if self.measurements.len() < 3 {
            return BandwidthTrend::Stable;
        }

        // Compare recent half vs older half
        let mid = self.measurements.len() / 2;
        let older_avg: u64 = self.measurements
            .iter()
            .take(mid)
            .map(|m| m.bytes_per_second)
            .sum::<u64>() / mid as u64;

        let recent_avg: u64 = self.measurements
            .iter()
            .skip(mid)
            .map(|m| m.bytes_per_second)
            .sum::<u64>() / (self.measurements.len() - mid) as u64;

        // 20% threshold for trend detection
        let threshold = older_avg / 5;

        if recent_avg > older_avg + threshold {
            BandwidthTrend::Increasing
        } else if recent_avg < older_avg.saturating_sub(threshold) {
            BandwidthTrend::Decreasing
        } else {
            BandwidthTrend::Stable
        }
    }

    /// Get metrics snapshot
    pub fn metrics(&self) -> BandwidthMetrics {
        self.metrics.clone()
    }
}
```

#### 3. AdaptiveQualityManager

**Purpose**: Analyze metrics and trigger quality adjustments.

**Implementation**: `src/player/adaptive_quality.rs`

```rust
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use crate::models::{QualityOption, Resolution};

#[derive(Debug, Clone, PartialEq)]
pub enum AdaptiveMode {
    Auto,       // Automatic quality adjustment
    Manual,     // User controls quality
}

#[derive(Debug)]
pub enum QualityDecision {
    Maintain,                         // Keep current quality
    Decrease(QualityOption),          // Switch to lower quality
    Increase(QualityOption),          // Switch to higher quality
    Recover(QualityOption),           // Emergency: playback failed, retry lower
}

pub struct AdaptiveQualityManager {
    mode: AdaptiveMode,
    available_qualities: Vec<QualityOption>,
    current_quality_index: usize,

    // Metrics receivers
    playback_rx: mpsc::Receiver<PlaybackMetrics>,
    bandwidth_rx: mpsc::Receiver<BandwidthMetrics>,

    // State
    last_quality_change: Option<Instant>,
    quality_change_cooldown: Duration,

    // Decision output
    decision_tx: mpsc::Sender<QualityDecision>,
}

impl AdaptiveQualityManager {
    pub fn new(
        available_qualities: Vec<QualityOption>,
        current_quality_index: usize,
        playback_rx: mpsc::Receiver<PlaybackMetrics>,
        bandwidth_rx: mpsc::Receiver<BandwidthMetrics>,
        decision_tx: mpsc::Sender<QualityDecision>,
    ) -> Self {
        Self {
            mode: AdaptiveMode::Auto,
            available_qualities,
            current_quality_index,
            playback_rx,
            bandwidth_rx,
            last_quality_change: None,
            quality_change_cooldown: Duration::from_secs(10),
            decision_tx,
        }
    }

    pub async fn run(&mut self) {
        let mut playback_metrics: Option<PlaybackMetrics> = None;
        let mut bandwidth_metrics: Option<BandwidthMetrics> = None;

        loop {
            tokio::select! {
                Some(metrics) = self.playback_rx.recv() => {
                    playback_metrics = Some(metrics);
                }

                Some(metrics) = self.bandwidth_rx.recv() => {
                    bandwidth_metrics = Some(metrics);
                }
            }

            // Only make decisions in Auto mode
            if self.mode != AdaptiveMode::Auto {
                continue;
            }

            // Need both metrics to make decisions
            if let (Some(ref playback), Some(ref bandwidth)) = (&playback_metrics, &bandwidth_metrics) {
                if let Some(decision) = self.evaluate_quality(playback, bandwidth) {
                    tracing::info!("Adaptive quality decision: {:?}", decision);
                    let _ = self.decision_tx.send(decision).await;
                }
            }
        }
    }

    fn evaluate_quality(
        &mut self,
        playback: &PlaybackMetrics,
        bandwidth: &BandwidthMetrics,
    ) -> Option<QualityDecision> {
        // Check cooldown period
        if let Some(last_change) = self.last_quality_change {
            if last_change.elapsed() < self.quality_change_cooldown {
                return None; // Too soon to change again
            }
        }

        let current_quality = &self.available_qualities[self.current_quality_index];

        // Emergency: Playback failed completely
        if matches!(playback.health, PlaybackHealth::Failed) {
            return self.emergency_recovery();
        }

        // Critical: Frequent buffering (unstable)
        if matches!(playback.health, PlaybackHealth::Unstable) {
            return self.decrease_quality_progressive();
        }

        // Check if current quality exceeds available bandwidth
        let required_bps = current_quality.bitrate;
        let available_bps = bandwidth.estimated_available_bps;

        if required_bps > available_bps {
            // Not enough bandwidth for current quality
            tracing::warn!(
                "Insufficient bandwidth: required {} Mbps, available {} Mbps",
                required_bps / 1_000_000,
                available_bps / 1_000_000
            );
            return self.decrease_quality_progressive();
        }

        // Opportunity: Bandwidth increasing and stable
        if matches!(bandwidth.trend, BandwidthTrend::Increasing | BandwidthTrend::Stable)
            && matches!(playback.health, PlaybackHealth::Healthy)
            && playback.buffer_count == 0
        {
            // Can we upgrade quality?
            return self.increase_quality_progressive(bandwidth.estimated_available_bps);
        }

        None // Maintain current quality
    }

    fn emergency_recovery(&mut self) -> Option<QualityDecision> {
        // Jump down significantly for recovery
        let target_index = if self.current_quality_index >= 2 {
            self.current_quality_index - 2 // Drop 2 levels
        } else {
            self.available_qualities.len() - 1 // Lowest quality
        };

        self.last_quality_change = Some(Instant::now());
        self.current_quality_index = target_index;

        Some(QualityDecision::Recover(
            self.available_qualities[target_index].clone()
        ))
    }

    fn decrease_quality_progressive(&mut self) -> Option<QualityDecision> {
        // Can we go lower?
        if self.current_quality_index >= self.available_qualities.len() - 1 {
            tracing::warn!("Already at lowest quality, cannot decrease");
            return None; // Already at lowest
        }

        // Drop one level
        self.current_quality_index += 1;
        self.last_quality_change = Some(Instant::now());

        Some(QualityDecision::Decrease(
            self.available_qualities[self.current_quality_index].clone()
        ))
    }

    fn increase_quality_progressive(&mut self, available_bps: u64) -> Option<QualityDecision> {
        // Can we go higher?
        if self.current_quality_index == 0 {
            return None; // Already at highest
        }

        // Check if next higher quality fits bandwidth (with 20% headroom)
        let next_index = self.current_quality_index - 1;
        let next_quality = &self.available_qualities[next_index];

        let required_with_headroom = (next_quality.bitrate * 120) / 100;

        if available_bps >= required_with_headroom {
            self.current_quality_index = next_index;
            self.last_quality_change = Some(Instant::now());

            Some(QualityDecision::Increase(next_quality.clone()))
        } else {
            None // Not enough bandwidth yet
        }
    }

    /// Set adaptive mode (called by user preference)
    pub fn set_mode(&mut self, mode: AdaptiveMode) {
        self.mode = mode;
        tracing::info!("Adaptive quality mode: {:?}", mode);
    }

    /// Manually set quality (disables auto mode)
    pub fn set_manual_quality(&mut self, index: usize) {
        if index < self.available_qualities.len() {
            self.mode = AdaptiveMode::Manual;
            self.current_quality_index = index;
        }
    }
}
```

### Integration with PlayerController

**Update**: `src/player/controller.rs`

```rust
use tokio::sync::mpsc;

pub struct PlayerController {
    // ... existing fields

    // Adaptive quality components
    playback_monitor: Option<PlaybackMonitor>,
    bandwidth_monitor: Arc<Mutex<BandwidthMonitor>>,
    adaptive_quality_manager: Option<AdaptiveQualityManager>,

    // Metrics channels
    playback_metrics_tx: mpsc::Sender<PlaybackMetrics>,
    bandwidth_metrics_tx: mpsc::Sender<BandwidthMetrics>,
    quality_decision_rx: Option<mpsc::Receiver<QualityDecision>>,
}

impl PlayerController {
    pub fn new(/* ... */) -> Self {
        let (playback_metrics_tx, playback_metrics_rx) = mpsc::channel(10);
        let (bandwidth_metrics_tx, bandwidth_metrics_rx) = mpsc::channel(10);
        let (quality_decision_tx, quality_decision_rx) = mpsc::channel(5);

        // Create bandwidth monitor (shared with cache proxy for measurements)
        let bandwidth_monitor = Arc::new(Mutex::new(
            BandwidthMonitor::new(bandwidth_metrics_tx.clone())
        ));

        Self {
            // ... existing fields
            playback_monitor: None,
            bandwidth_monitor,
            adaptive_quality_manager: None,
            playback_metrics_tx,
            bandwidth_metrics_tx,
            quality_decision_rx: Some(quality_decision_rx),
        }
    }

    pub async fn play(&mut self, media_id: MediaItemId, stream_info: StreamInfo) -> Result<()> {
        // ... existing play logic

        // Start playback monitor
        let (state_tx, state_rx) = mpsc::channel(20);
        let playback_monitor = PlaybackMonitor::new(
            state_rx,
            self.playback_metrics_tx.clone(),
        );

        tokio::spawn(async move {
            playback_monitor.run().await;
        });

        // Start adaptive quality manager
        let (playback_rx, bandwidth_rx) = /* get receivers from somewhere */;
        let adaptive_manager = AdaptiveQualityManager::new(
            stream_info.quality_options.clone(),
            0, // Start with highest quality
            playback_rx,
            bandwidth_rx,
            quality_decision_tx,
        );

        tokio::spawn(async move {
            adaptive_manager.run().await;
        });

        // Monitor quality decisions
        let decision_rx = self.quality_decision_rx.take().unwrap();
        self.spawn_quality_decision_handler(decision_rx);

        Ok(())
    }

    fn spawn_quality_decision_handler(&self, mut rx: mpsc::Receiver<QualityDecision>) {
        let controller = /* self reference or channel */;

        tokio::spawn(async move {
            while let Some(decision) = rx.recv().await {
                match decision {
                    QualityDecision::Decrease(quality) => {
                        tracing::info!("AUTO: Decreasing quality to {}", quality.name);
                        // Trigger quality change
                    }

                    QualityDecision::Increase(quality) => {
                        tracing::info!("AUTO: Increasing quality to {}", quality.name);
                        // Trigger quality change
                    }

                    QualityDecision::Recover(quality) => {
                        tracing::warn!("AUTO: Emergency recovery to {}", quality.name);
                        // Trigger quality change with recovery flag
                    }

                    QualityDecision::Maintain => {
                        // Do nothing
                    }
                }
            }
        });
    }

    /// Hook for cache proxy to report chunk downloads
    pub async fn record_chunk_download(&self, bytes: u64, duration: Duration) {
        self.bandwidth_monitor
            .lock()
            .await
            .record_download(bytes, duration)
            .await;
    }
}
```

### CacheProxy Integration

**Update**: `src/cache/proxy.rs`

The cache proxy should report chunk download times to the bandwidth monitor:

```rust
async fn download_chunk(&self, entry_id: i32, chunk_index: u64) -> Result<()> {
    let start = Instant::now();

    // ... existing download logic

    let duration = start.elapsed();
    let bytes = /* chunk size */;

    // Report to bandwidth monitor
    if let Some(controller) = &self.player_controller {
        controller.record_chunk_download(bytes, duration).await;
    }

    Ok(())
}
```

### UI Indicators

**Update**: `src/ui/pages/player.rs`

Add visual indicators for adaptive quality:

```rust
view! {
    gtk::Box {
        // ... existing player UI

        // Adaptive quality indicator
        gtk::Box {
            set_visible: model.adaptive_quality_active,
            set_halign: gtk::Align::Start,
            set_valign: gtk::Align::Start,
            add_css_class: "adaptive-quality-indicator",

            gtk::Image {
                set_icon_name: Some("network-wireless-signal-good-symbolic"),
            },

            gtk::Label {
                #[watch]
                set_label: &format!("Auto ({})", model.current_quality_name),
                add_css_class: "caption",
            },
        }
    }
}
```

### User Preferences

Add settings to enable/disable adaptive quality:

- **Auto mode**: System adjusts quality automatically
- **Manual mode**: User selects quality, no automatic changes
- **Aggressive**: Quick quality changes (5s cooldown)
- **Conservative**: Slower quality changes (15s cooldown)
- **Minimum quality**: Prevent dropping below certain threshold

### Quality Adjustment Algorithm

#### Decision Matrix

| Condition | Action | Reason |
|-----------|--------|--------|
| Playback failed | Drop 2 levels or lowest | Emergency recovery |
| 3+ buffers in 60s | Drop 1 level | Unstable playback |
| Bandwidth < required | Drop 1 level | Insufficient bandwidth |
| Bandwidth > required + 20% headroom | Raise 1 level | Opportunity to improve |
| Healthy + no buffers for 30s | Consider raise | Stable, can try higher |

#### Progressive Changes

- **Never jump more than 2 quality levels** (except emergency)
- **Cooldown period**: 10 seconds between changes
- **Hysteresis**: Require 20% bandwidth headroom to upgrade
- **Downgrade faster than upgrade**: Safety first

#### Example Scenario

```
00:00 - Start playback at 1080p (8 Mbps)
00:15 - Bandwidth drops to 5 Mbps
00:18 - Buffer event detected
00:20 - AUTO: Decrease to 720p (4 Mbps)
00:35 - Playback stable, bandwidth 6 Mbps
01:00 - Bandwidth stable at 9 Mbps for 25s
01:05 - AUTO: Increase to 1080p (8 Mbps)
01:20 - Playback continues smoothly
```

## Implementation Phases

### Phase 1: Backend Foundation (Task 209.1)

**Goal**: Enable decision endpoint and connection type detection

**Tasks**:
1. Implement `is_local_connection()` in `PlexBackend` using `ConnectionService::cache()`
2. Implement `get_connection_location()` helper method
3. Create `src/backends/plex/api/decision.rs`
4. Implement `get_stream_url_via_decision()`
5. Add tests for decision endpoint

**Acceptance Criteria**:
- [ ] PlexBackend queries ConnectionService for connection type
- [ ] Decision endpoint successfully requests streams
- [ ] Decision endpoint handles direct play and transcode modes
- [ ] Connection location (lan/wan) correctly determined from ConnectionCache
- [ ] No duplicate connection tracking logic (uses existing ConnectionService)

**Files**:
- `src/backends/plex/mod.rs` (add helper methods)
- `src/backends/plex/api/decision.rs` (new)
- `src/backends/plex/api/mod.rs` (exports)

### Phase 2: Quality Selection Logic (Task 209.2)

**Goal**: Update stream URL generation to support quality options

**Tasks**:
1. Enhance `get_stream_url()` to return quality options in `StreamInfo`
2. Implement `get_stream_url_for_quality()`
3. Add logic to choose direct URL vs decision endpoint based on quality
4. Update `StreamInfo` model if needed to store quality options

**Acceptance Criteria**:
- [ ] `get_stream_url()` returns quality options array
- [ ] Quality options include original + transcoded variants
- [ ] `get_stream_url_for_quality()` generates correct URLs
- [ ] Direct play uses direct URLs, transcoded uses decision endpoint

**Files**:
- `src/backends/plex/api/streaming.rs`
- `src/models/mod.rs` (if StreamInfo needs updates)

### Phase 3: Cache Integration (Task 209.3)

**Goal**: Make file cache quality-aware

**Tasks**:
1. Update `FileCache::get_cached_stream_with_quality()`
2. Implement quality-based cache key generation
3. Verify cache_entries schema supports quality field (already does!)
4. Test multiple qualities cached simultaneously

**Acceptance Criteria**:
- [ ] Different qualities cached separately
- [ ] Cache lookup uses (source_id, media_id, quality) key
- [ ] Can cache original + 1080p + 720p simultaneously
- [ ] Chunk-based downloads work for transcoded streams

**Files**:
- `src/cache/file_cache.rs`
- `src/db/repository/cache_repository.rs`

### Phase 4: Quality Selector UI (Task 209.4)

**Goal**: Create UI component for quality selection

**Tasks**:
1. Create `QualitySelector` component
2. Implement dropdown with quality options
3. Add quality info label (resolution, bitrate)
4. Handle quality change events
5. Add to player controls overlay

**Acceptance Criteria**:
- [ ] Quality dropdown shows all available options
- [ ] Current quality displayed with resolution and bitrate
- [ ] Quality change event propagates to player
- [ ] UI updates when new media loads

**Files**:
- `src/ui/shared/quality_selector.rs`
- `src/ui/shared/mod.rs`

### Phase 5: MediaService Integration (Task 209.5)

**Goal**: Wire quality selection to backend and cache

**Tasks**:
1. Add `get_stream_with_quality()` to MediaService
2. Update PlayerPage to use quality-aware stream fetching
3. Handle quality changes during playback
4. Add loading states for quality switching

**Acceptance Criteria**:
- [ ] Quality selection triggers stream URL fetch
- [ ] Cache lookup uses correct quality key
- [ ] Player switches to new quality smoothly
- [ ] Loading indicator shows during quality change

**Files**:
- `src/services/core/media.rs`
- `src/ui/pages/player.rs`

### Phase 6: Remote Connection Handling (Task 209.6)

**Goal**: Ensure remote connections always use decision endpoint

**Tasks**:
1. Update `PlexBackend::get_stream_url()` to query ConnectionService for connection type
2. For remote connections, force decision endpoint even for "original" quality
3. Add fallback logic: try direct URL, fall back to decision endpoint
4. Log connection type and URL generation method

**Acceptance Criteria**:
- [ ] Local connections use direct URLs (faster)
- [ ] Remote connections use decision endpoint (required)
- [ ] Relay connections work correctly
- [ ] Fallback to decision endpoint on direct URL failure
- [ ] ConnectionService cache consulted for connection type determination

**Files**:
- `src/backends/plex/mod.rs`
- `src/backends/plex/api/streaming.rs`
- `src/services/core/connection.rs` (reference only)

### Phase 7: Testing & Polish (Task 209.7)

**Goal**: Comprehensive testing and UX improvements

**Tasks**:
1. Test local playback (should use direct URLs)
2. Test remote playback (should use decision endpoint)
3. Test quality switching during playback
4. Test cache with multiple qualities
5. Add error handling for decision endpoint failures
6. Add retry logic for transient failures
7. Update user-facing error messages

**Acceptance Criteria**:
- [ ] All quality options work on local connection
- [ ] All quality options work on remote connection
- [ ] Quality switching during playback is smooth (<3 second interruption)
- [ ] Multiple qualities can be cached simultaneously
- [ ] Error messages are user-friendly
- [ ] Logs provide debugging information

**Files**:
- All modified files
- `tests/` (integration tests)

### Phase 8: Adaptive Quality & Smart Recovery (Task 209.8)

**Goal**: Automatic quality adjustment and intelligent playback recovery

**Tasks**:
1. Implement `PlaybackMonitor` to track player health
2. Add `BandwidthMonitor` to measure download speeds
3. Create `AdaptiveQualityManager` for automatic quality adjustment
4. Implement smart recovery for playback failures
5. Add progressive quality degradation/improvement
6. Create UI indicators for adaptive quality changes
7. Add user preferences for adaptive quality settings

**Acceptance Criteria**:
- [ ] System detects buffering and playback stalls
- [ ] Bandwidth continuously monitored during playback
- [ ] Quality automatically decreases when buffering occurs
- [ ] Quality automatically increases when bandwidth improves
- [ ] Quality changes are progressive (not extreme jumps)
- [ ] Failed playback automatically retries with lower quality
- [ ] User can override adaptive quality (manual mode)
- [ ] UI shows when adaptive quality is active

**Files**:
- `src/player/adaptive_quality.rs` (new)
- `src/player/playback_monitor.rs` (new)
- `src/player/bandwidth_monitor.rs` (new)
- `src/player/controller.rs` (update)
- `src/ui/pages/player.rs` (update)
- `src/services/core/media.rs` (update)

## Database Changes

### No Migration Required! ✅

The existing `cache_entries` table already has the `quality` field:

```sql
CREATE TABLE cache_entries (
    id INTEGER PRIMARY KEY,
    source_id TEXT NOT NULL,
    media_id TEXT NOT NULL,
    quality TEXT NOT NULL,  -- ← Already exists!
    original_url TEXT NOT NULL,
    file_path TEXT NOT NULL,
    expected_total_size INTEGER,
    is_complete BOOLEAN DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_accessed TIMESTAMP,
    UNIQUE(source_id, media_id, quality)  -- ← Already has unique constraint!
);
```

This was included in the original cache schema but not actively used. Perfect! 🎉

## Quality Naming Convention

### Quality Keys

For cache lookup and storage:

| Quality | Key String | Resolution | Bitrate |
|---------|-----------|------------|---------|
| Original (Direct Play) | `"original"` | Source | Source |
| 1080p Transcode | `"1080p"` | 1920x1080 | 8 Mbps |
| 720p Transcode | `"720p"` | 1280x720 | 4 Mbps |
| 480p Transcode | `"480p"` | 854x480 | 2 Mbps |
| 360p Transcode | `"360p"` | 640x360 | 1 Mbps |

### URL Generation

**Local Connection + Original Quality**:
```
{base_url}/library/metadata/{id}/file?X-Plex-Token={token}
```

**Remote Connection + Original Quality**:
```
{base_url}/video/:/transcode/universal/decision?path=/library/metadata/{id}&mediaIndex=0&partIndex=0&protocol=http&directPlay=1&directStream=1&hasMDE=1&location=wan&X-Plex-Token={token}
```

**Any Connection + Transcoded Quality**:
```
{base_url}/video/:/transcode/universal/start.m3u8?path=/library/metadata/{id}&mediaIndex=0&partIndex=0&protocol=hls&directPlay=0&directStream=0&maxVideoBitrate={bitrate_kbps}&videoResolution={width}x{height}&fastSeek=1&location={lan|wan}&X-Plex-Token={token}
```

## User Experience Flow

### Initial Playback

1. User clicks "Play" on movie/episode
2. System fetches `StreamInfo` with quality options
3. Quality selector populates with options (default: Original)
4. System determines connection type (local vs remote)
5. Generates appropriate stream URL
6. Cache system checks for cached content
7. Player starts with proxy URL
8. Chunks download progressively

**Timeline**: 100-500ms to start playback

### Quality Change

1. User selects different quality from dropdown
2. UI shows loading indicator
3. System generates new stream URL for selected quality
4. Cache checks for existing chunks at new quality
5. Player receives new URL and switches
6. Playback continues from same position
7. Chunks download for new quality

**Timeline**: 1-3 seconds interruption

### Seek During Quality Change

1. If quality change is in progress, cancel pending requests
2. New quality + new position both applied
3. Prioritize chunks at seek position with new quality
4. Resume playback when chunk available

## Performance Considerations

### Cache Storage

For a 4GB movie with 3 qualities cached:
- Original: 4GB
- 1080p: ~3.5GB (similar to original)
- 720p: ~1.8GB
- **Total**: ~9.3GB

**Recommendation**: Add cache size limits and cleanup policies (future enhancement)

### Network Bandwidth

Transcoded streams reduce bandwidth for remote playback:
- Original 4K: 20-40 Mbps
- 1080p: 8 Mbps (75% reduction)
- 720p: 4 Mbps (87% reduction)

### Chunk Download Priority

When quality changes:
1. Cancel LOW priority downloads for old quality
2. Request chunks for new quality with HIGH priority
3. Current playback position always CRITICAL priority

## Error Handling

### Decision Endpoint Failures

| Error | Cause | Fallback |
|-------|-------|----------|
| 401 Unauthorized | Invalid token | Re-authenticate |
| 503 Service Unavailable | Server transcoder busy | Retry or suggest lower quality |
| Timeout | Network issues | Retry with exponential backoff |
| Invalid response | Parsing error | Fall back to direct URL if local |

### Quality Switch Failures

1. If decision endpoint fails → show error, keep current quality
2. If cache fails → direct stream from original URL (passthrough)
3. If player fails → reload player with fallback quality

## Testing Strategy

### Unit Tests

- `decision.rs`: Test decision endpoint request/response parsing
- `streaming.rs`: Test quality option generation
- `file_cache.rs`: Test quality-based cache keys

### Integration Tests

- Local playback with all qualities
- Remote playback with all qualities
- Quality switching during playback
- Cache persistence across quality changes
- Simultaneous playback of different qualities

### Manual Testing Checklist

- [ ] Play movie locally, original quality
- [ ] Switch to 720p, verify transcode URL
- [ ] Seek during playback, verify chunks download
- [ ] Stop, restart app, verify cache survives
- [ ] Connect remotely (WAN), verify decision endpoint used
- [ ] Try all qualities on remote connection
- [ ] Switch qualities multiple times rapidly
- [ ] Test with slow network connection
- [ ] Test with relay connection
- [ ] Verify cache storage uses quality keys

## Success Criteria

### Core Quality Selection

1. ✅ Remote Plex playback works via decision endpoint
2. ✅ Users can select quality from UI dropdown
3. ✅ Different qualities cached separately
4. ✅ Quality switching works during playback
5. ✅ Local connections use fast direct URLs
6. ✅ Chunk-based cache works with transcoded streams
7. ✅ No regressions in existing playback functionality

### Adaptive Quality & Smart Recovery

8. ✅ Playback monitor detects buffering and health issues
9. ✅ Bandwidth monitor tracks download speeds and trends
10. ✅ Quality automatically decreases when buffering occurs
11. ✅ Quality automatically increases when bandwidth improves
12. ✅ Quality changes are progressive (maximum 2 levels at once)
13. ✅ Failed playback triggers emergency quality recovery
14. ✅ User can toggle between Auto and Manual mode
15. ✅ UI indicates when adaptive quality is active
16. ✅ Bandwidth and quality info displayed in UI
17. ✅ Cooldown period prevents rapid quality oscillation (10s minimum)
18. ✅ System requires 20% bandwidth headroom before upgrading quality

## References

- **Research**: task-367 (Plex decision endpoint investigation)
- **Implementation**: task-209 (This work)
- **Cache System**: `docs/file-cache.md`
- **Player Architecture**: `src/player/`
- **Plex API**: https://plexapi.dev/api-reference/video/start-universal-transcode
- **Python PlexAPI**: https://github.com/pkkid/python-plexapi/blob/master/plexapi/base.py

## Future Enhancements

Beyond Phase 8, potential improvements include:

1. **Pre-cache Quality Selection**: Allow users to select which quality to pre-download for offline viewing
2. **Quality Profiles per Network**: Save user preferences for different network types (Home WiFi, Mobile, Work, etc.)
3. **Quality Preview Thumbnails**: Show thumbnail previews at different qualities before switching
4. **Smart Pre-caching**: Automatically pre-cache lower quality for offline, stream higher quality when online
5. **Machine Learning**: Predict buffering events based on historical patterns
6. **CDN Integration**: Use content delivery networks for faster chunk delivery
7. **Network Type Detection**: Automatically detect WiFi vs cellular and adjust quality accordingly
8. **Advanced Bandwidth Prediction**: Use more sophisticated algorithms to predict bandwidth changes
9. **Battery-aware Quality**: Reduce quality on battery power to extend device runtime
10. **Time-of-day Profiles**: Different quality settings for different times (e.g., lower quality during peak hours)
