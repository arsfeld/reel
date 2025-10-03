---
id: task-360
title: Integrate CacheCleanupWorker with ConfigService
status: To Do
assignee: []
created_date: '2025-10-03 14:34'
labels:
  - cache
  - worker
  - config
dependencies: []
priority: medium
---

## Description

The CacheCleanupWorker currently uses default FileCacheConfig instead of loading from ConfigService. It should load config at startup and subscribe to config updates via MessageBroker to react to runtime changes. This aligns with the Relm4 reactive pattern used by other components.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Load initial cache config from disk at worker startup using Config::load()
- [ ] #2 Subscribe to MessageBroker for config update events
- [ ] #3 Handle BrokerMessage::Config(ConfigMessage::Updated) in worker Input
- [ ] #4 Update worker's cache_config when config changes
- [ ] #5 Recalculate dynamic cache limits after config update
- [ ] #6 Restart cleanup timer if cleanup_interval changes
<!-- AC:END -->
