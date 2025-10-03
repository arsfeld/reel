---
id: task-365
title: Add local vs remote connection labels in logs
status: To Do
assignee: []
created_date: '2025-10-03 16:09'
labels:
  - enhancement
  - logging
  - networking
dependencies: []
priority: medium
---

## Description

Connection logs don't clearly distinguish between local network connections and remote (relay/cloud) connections. This makes debugging connection issues difficult. Add clear labeling in logs to identify connection types based on IP patterns and plex.direct subdomain structure.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Implement function to detect if connection is local (192.168.x.x, 10.x.x.x, 172.16-31.x.x)
- [ ] #2 Implement function to detect relay connections (specific plex.direct patterns)
- [ ] #3 Add '[LOCAL]' or '[REMOTE]' prefix to connection log messages
- [ ] #4 Update all connection-related log statements to include connection type
- [ ] #5 Logs clearly show whether each connection attempt is local or remote
<!-- AC:END -->
