# Warning Remediation Plan

## Objectives
- Drive the build to a zero-warning baseline without introducing compile failures.
- Prefer deleting unused code and data structures over suppressing warnings.
- Keep the codebase shippable at all times by running a fresh `cargo check` after each batch of changes.

## Baseline & Tracking
1. Run `nix develop --command cargo check --message-format short` to capture the full warning set and save it as `warnings-baseline.log` for reference.
2. Post-fix, repeat the command and diff against the previous log to verify progress and to detect any new warnings early.
3. Once the warning count approaches zero, add a `cargo clean` + full rebuild pass to confirm nothing lingers in incremental artifacts.

## Prioritized Workstreams
1. **Hygiene pass (imports & simple items)**
   - Target obvious `unused import`, `unused variable`, and `allow(unused)` remnants across the tree (e.g., `src/services/core/playback.rs`).
   - These are quick wins that immediately shrink duplicate warning noise and make later passes clearer.
2. **Backend API cleanup (`src/backends/**`)**
   - Jellyfin and Plex modules account for the largest cluster of `dead_code` warnings (constructors, DTO fields, helper methods).
   - Decide case-by-case whether to wire the functionality into the service layer or delete the unused pieces. Default to removal unless an immediate integration plan exists.
3. **Service layer rationalization (`src/services/core/**` & repositories)**
   - Address unused async functions (e.g., `BackendService::test_connection`, `MediaService` fetch helpers) by either
     - promoting them to the platform layer if they represent planned features, or
     - removing them alongside associated repository queries when no caller exists.
   - Verify that any deleted functions have no references in UI factories or workers.
4. **UI (Relm4) components**
   - Many message enums and struct fields are never constructed (`main_window.rs`, `home.rs`, `broker.rs`).
   - Align each component’s `Msg`/`Command` definitions with current UI flows and delete dormant variants and state fields.
   - Confirm factory/worker channel wiring still covers the active cases after pruning.
5. **Workers & Player modules**
   - Clean up unused worker commands (`image_loader`, `search_worker`, `sync_worker`) and player controller operations (`SetUpscalingMode`, `Shutdown`, etc.).
   - When functionality is on the roadmap, capture TODOs in Backlog before removal; otherwise drop the code to honor the “dead code must be removed” directive.
6. **Final sweep & linting**
   - Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` once the compiler is quiet to ensure style and semantics remain intact.
   - If any warnings persist, resolve them individually—no `#[allow]` unless the warning is false-positive and justified in review.

## Working Cadence
- Work one directory (or logical feature slice) at a time.
- After each slice:
  - Run `nix develop --command cargo check --message-format short` to ensure zero warnings in the touched scope.
  - Stage changes once they compile cleanly; keep commits focused per workstream for easier review.
- Maintain a running changelog in the relevant Backlog task notes as functionality is removed or moved.

## Risk Mitigation
- Deleting unused service methods or DTO fields can cascade—search for indirect usage (tests, serde derives, trait impls) before removal.
- For generated API models, confirm that serde expectations (`#[serde(rename)]`, etc.) are not needed for deserialization of still-used paths.
- When pruning UI messages, ensure matching handler branches and factory wiring are updated to avoid runtime panics on unmatched messages.

## Definition of Done
- `cargo clean && nix develop --command cargo check --message-format short` emits zero warnings.
- `nix develop --command cargo clippy -- -D warnings` passes.
- Full `cargo test` succeeds.
- All removed code is reflected in documentation or Backlog notes where relevant, and no regressions are observed in manual smoke checks of the desktop app.
