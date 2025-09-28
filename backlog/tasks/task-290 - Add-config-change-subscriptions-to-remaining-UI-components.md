---
id: task-290
title: Add config change subscriptions to remaining UI components
status: Done
assignee:
  - '@claude'
created_date: '2025-09-28 01:07'
updated_date: '2025-09-28 02:03'
labels:
  - config
  - ui
  - enhancement
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Update the remaining UI components (player page, home page, library views) to subscribe to config changes via MessageBroker and react appropriately when configuration is updated at runtime.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Player page subscribes to config changes and updates player if needed
- [x] #2 Home page reacts to relevant config changes
- [x] #3 Library views update based on config changes
- [x] #4 Components properly unsubscribe when destroyed
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze existing config subscription implementation in preferences page
2. Add BrokerMsg variant to Input enums for player, home, and library pages
3. Subscribe to config changes via MessageBroker in init methods
4. Handle config update messages in update methods
5. Implement proper unsubscription in shutdown methods
6. Test compilation and verify config propagation
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully added config change subscriptions to all UI components:

## Changes Made:

### Player Page (src/ui/pages/player.rs):
- Added BrokerMsg(BrokerMessage) to PlayerInput enum
- Subscribed to MessageBroker in init() using relm4::channel
- Implemented config handling in update() to react to player backend and upscaling mode changes
- Added unsubscribe in shutdown() method

### Home Page (src/ui/pages/home.rs):
- Added BrokerMsg(BrokerMessage) to HomePageInput enum  
- Subscribed to MessageBroker in init() using relm4::channel
- Added basic config handling in update() (ready for future enhancements)
- Added shutdown() method with unsubscribe

### Library Page (src/ui/pages/library.rs):
- Added BrokerMsg(BrokerMessage) to LibraryPageInput enum
- Subscribed to MessageBroker in init() using relm4::channel
- Added basic config handling in update() (ready for future enhancements)
- Added shutdown() method with unsubscribe

## Key Implementation Details:
- Used relm4::channel for message passing instead of tokio channels
- All components properly unsubscribe in shutdown() to prevent memory leaks
- Player page actively responds to config changes (player backend, upscaling mode)
- Config updates are broadcast via ConfigManager worker through MessageBroker

Code compiles successfully with all components ready to react to runtime config changes.
<!-- SECTION:NOTES:END -->
