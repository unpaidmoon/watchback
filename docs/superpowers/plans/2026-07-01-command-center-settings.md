# Command Center Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the command center around player identity, data actions, and user settings while removing the dedicated sync page from navigation.

**Architecture:** Keep `/onboarding` as the command center route. Add small Tauri commands for non-sensitive profile/settings/update/logout state, leave match sync as an action inside the command center, and remove `/sync` from the top navigation and QA region list.

**Tech Stack:** React, TypeScript, Tauri Rust commands, JSON file-backed local settings/profile, existing local sync/data APIs.

---

### Task 1: Test Navigation And API Surface

- [x] Add frontend tests proving the sync nav item is gone and new settings/profile API wrappers exist.
- [x] Run the test and confirm it fails before implementation.
- [x] Add TypeScript wrappers and shared types.
- [x] Re-run the test until it passes.

### Task 2: Backend Commands

- [x] Add Rust tests around logout removing a credential file and settings defaulting to system theme.
- [x] Run targeted Rust tests and confirm they fail before implementation.
- [x] Implement profile, settings, logout, and update-check command support.
- [x] Register commands in Tauri invoke handler.
- [x] Re-run Rust tests until green.

### Task 3: Command Center UI

- [x] Refactor command center modules into player profile, data actions, and settings.
- [x] Remove `SyncPage` import and `/sync` route.
- [x] Remove the sync nav item and sync QA page.
- [x] Add CSS for the new player card and settings controls.

### Task 4: Cleanup Verification

- [x] Delete the unused sync page after route removal.
- [x] Search for stale `/sync`, `SyncPage`, and old QA region references.
- [x] Run frontend tests, TypeScript build, and Rust tests.
