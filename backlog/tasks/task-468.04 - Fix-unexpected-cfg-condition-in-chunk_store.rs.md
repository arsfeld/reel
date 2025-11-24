---
id: task-468.04
title: Fix unexpected cfg condition in chunk_store.rs
status: Done
assignee: []
created_date: '2025-11-24 19:57'
updated_date: '2025-11-24 20:11'
labels:
  - cleanup
dependencies: []
parent_task_id: task-468
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Fix the unexpected cfg condition in src/cache/chunk_store.rs:12:
```
#[cfg(feature = "storage_full_error")]
```

The feature "storage_full_error" is not defined in Cargo.toml. Either:
1. Add the feature to Cargo.toml if it's needed
2. Remove the cfg attribute and the conditional code if it's not needed
<!-- SECTION:DESCRIPTION:END -->
