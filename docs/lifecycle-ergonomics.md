# Lifecycle Ergonomics (Dec 2025)

This document summarizes the lifecycle ergonomics refactor captured in `.specman/scratchpad/lifecycle-ergonomics/scratch.md`.

## What Changed

- **New library façade:** The `specman` crate now exposes a high-level `Specman` service façade (plus a `DefaultSpecman` constructor) that unifies lifecycle flows (plan/create/delete) behind a single entrypoint.
- **Structured lifecycle errors:** Deletion failures that callers often need to branch on are now programmatic:
  - `SpecmanError::Lifecycle(LifecycleError::DeletionBlocked { .. })`
  - `SpecmanError::Lifecycle(LifecycleError::PlanTargetMismatch { .. })`
- **Legacy helpers removed:** The older `api.rs` convenience helpers were removed; the façade is the ergonomic entrypoint.
- **Create planning now tolerates new targets:** `DefaultLifecycleController::plan_creation` no longer fails for brand-new artifacts that don’t exist on disk yet; it plans with an empty dependency tree so callers can render/persist in one flow.
- **CLI migrated to façade:** `specman-cli` creation and deletion flows now use the façade request types (`CreateRequest`, `DeleteRequest`, `DeletePolicy`) and map lifecycle errors deterministically to exit statuses.
- **MCP prompt determinism restored:** Scratch prompt examples (branch/name) were made deterministic again and unit tests were updated.

## Why It Matters

- **Less wiring for callers:** CLI/MCP no longer need to manually compose lifecycle controller + template catalog + persistence for common flows.
- **Better automation hooks:** Callers can distinguish “blocked deletion” vs “plan mismatch” without brittle string matching.
- **More consistent create flows:** Planning can happen before a file exists, which matches real creation workflows.

## Quick Migration Notes

- If you previously used `specman::api::*` helpers, migrate to the façade:
  - Create: `Specman::plan_create` / `Specman::persist_rendered` (or `Specman::create`)
  - Delete: `Specman::delete` using `DeleteRequest` and `DeletePolicy`
- If you were parsing deletion failure strings, switch to matching `SpecmanError::Lifecycle(..)`.
