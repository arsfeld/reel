---
id: task-468.01
title: Remove unused imports
status: Done
assignee: []
created_date: '2025-11-24 19:57'
updated_date: '2025-11-24 20:03'
labels:
  - cleanup
dependencies: []
parent_task_id: task-468
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Remove all unused import statements across the codebase. Files affected:
- src/cache/proxy.rs (head, Context as TaskContext, Poll, AsyncSeekExt)
- src/db/repository/cache_repository.rs (cache_statistics)
- src/db/repository/media_repository.rs (JoinType, QuerySelect, RelationTrait, DatabaseBackend, QueryStatementBuilder)
- src/db/repository/people_repository.rs (self)
- src/player/gstreamer/sink_factory.rs (warn)
- src/player/gstreamer_player.rs (StreamInfo)
- src/services/cache_service.rs (info)
- src/services/conflict_resolver.rs (MediaItemId, std::sync::Arc, std::time::Duration)
- src/services/core/backend.rs (ChapterMarker)
- src/ui/main_window/navigation.rs (gtk::gio, SearchPage, SearchWorkerInput)
- src/ui/main_window/workers.rs (SourceId, PlaybackSyncWorkerInput, SearchWorkerInput, SyncWorkerInput)
- src/ui/main_window/mod.rs (AuthDialogInput, PreferencesDialogInput, PreferencesDialogOutput, BROKER, BrokerMessage, SourceMessage, ConnectionMonitorOutput, SearchWorkerOutput, SyncWorkerOutput)
- src/ui/pages/library/mod.rs (relm4::view, error, MediaItemId, ImageLoaderInput, ImageRequest, ImageSize)
- src/ui/pages/player/sleep_inhibition.rs (relm4::gtk)
- src/ui/pages/player/controls_visibility.rs (libadwaita as adw)
- src/ui/pages/player/menu_builders.rs (libadwaita as adw)
- src/ui/pages/player/buffering_overlay.rs (libadwaita as adw)
- src/ui/pages/player/mod.rs (BufferingOverlayInput)
- src/ui/pages/search.rs (media_items)
- src/ui/pages/show_details.rs (create_sync_status_indicator)
- src/ui/shared/sync_status.rs (libadwaita as adw)
- src/workers/cache_cleanup_worker.rs (Repository - 2 places)
- src/workers/playback_sync_worker.rs (SourceId)
- src/workers/search_worker.rs (MediaItemModel)
<!-- SECTION:DESCRIPTION:END -->
