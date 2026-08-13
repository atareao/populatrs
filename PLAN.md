# Two-step X (Twitter) Publishing with Reply Thread — Implementation Plan

## Overview

Add a `reply_template` field to X publisher config and implement a two-step publish flow: post the main tweet (title only), then reply to it with the URL + CTA. Backend is already implemented; this plan covers the frontend changes to configure the reply template.

## Tasks

### Task 1: Add `reply_template` to PublisherConfigEntry type

**Files:**
- Modify: `frontend/src/api/http.ts`

- [ ] **Step 1:** Add `reply_template?: string` to the config intersection in `PublisherConfigEntry`
      Find the `PublisherConfigEntry` type and add the field to the config object type so TypeScript accepts it in forms.

### Task 2: Add reply_template form field to PublisherList

**Files:**
- Modify: `frontend/src/pages/Publishers/PublisherList.tsx`

- [ ] **Step 1:** Add a `Form.Item` for `reply_template` after the universal template field
      Only render it when `publisherType === 'x'`. Use an `Input.TextArea` with `rows={2}` and placeholder like `"{{ url }} — {{ title | truncate(200) }}"`.

- [ ] **Step 2:** Wire the field into the save payload
      Ensure `reply_template` is included in the PUT request body when saving publisher config.

## Quality Gates

- [ ] **Backend:** `cargo fmt --check && cargo clippy -- -D warnings && cargo test` — all pass
- [ ] **Frontend:** `pnpm build && pnpm test` — build succeeds, tests pass

## Git Flow

- Branch: `feature/x-reply-thread` from `development`
- Commit: `✨ feat: two-step X publishing with reply thread`
- PR: `feature/x-reply-thread` → `development`
- Release: PR `development` → `main`