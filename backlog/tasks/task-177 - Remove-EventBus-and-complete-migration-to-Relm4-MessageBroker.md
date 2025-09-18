---
id: task-177
title: Remove EventBus and complete migration to Relm4 MessageBroker
status: Done
assignee: []
created_date: '2025-09-18 14:49'
updated_date: '2025-09-18 14:55'
labels:
  - refactoring
  - architecture
  - high-priority
dependencies: []
priority: high
---

## Description

The application currently has a legacy EventBus system in src/events/ that should be fully removed in favor of Relm4's built-in MessageBroker for inter-component communication. This will eliminate redundancy and fully embrace the Relm4 reactive architecture.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Identify all remaining EventBus usage in the codebase
- [ ] #2 Replace EventBus subscriptions with MessageBroker subscriptions
- [ ] #3 Convert event types to proper Relm4 messages
- [ ] #4 Update all components to use MessageBroker instead of EventBus
- [ ] #5 Remove src/events/event_bus.rs and related event infrastructure
- [ ] #6 Ensure all inter-component communication still works
- [ ] #7 Update repository layer to emit messages through MessageBroker
- [ ] #8 Test that all UI updates and background tasks communicate properly
- [ ] #9 Remove EventBus dependencies from Cargo.toml if any
<!-- AC:END -->
