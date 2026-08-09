# Upstream PR draft: Send Git diff review feedback to agent threads

Use this short description when reopening the upstream contribution. Attach a
screen recording that follows the checklist below.

## History hygiene (required when porting upstream)

Do **not** add any form of:

```text
Co-authored-by: Cursor <cursoragent@cursor.com>
```

Strip that trailer (and any Cursor / agent co-author trailers) from every commit
before opening or updating a `zed-industries/zed` pull request. Keep authorship
clean so upstream history is not polluted by editor-agent metadata.

## Title

Send Git diff review feedback to agent threads

## Body

```markdown
## Summary

- Let users leave draft comments on local Git diffs (project / branch / staged / unstaged).
- Send those comments to the active agent thread as structured cards (file, lines, excerpt, comment).
- Machine-readable JSON stays folded under a details block for agents that want it.

Related to https://github.com/zed-industries/zed/issues/59157

## How to try it

1. Open an unstaged or project diff with the `diff-review` feature flag enabled.
2. Hover the gutter, add a comment (drag to select a range).
3. Edit or delete drafts from the thin inline thread.
4. Click **Send to Agent (N)** in the toolbar.
5. Confirm the agent thread shows a readable card, not a raw JSON wall.
6. Click a backticked file path in the card to jump back into the project.

## Testing

- `cargo test -p editor test_review_feedback_preserves_anchor_metadata_until_cleared -- --nocapture`
- `cargo test -p agent_ui formats_review_feedback_as_structured_cards -- --nocapture`
- Manual: Light and Dark screenshots of the inline thread + agent card

## Showcase

_Attach: short screen recording of gutter comment → Send to Agent → card → jump back._

Release Notes:

- Added the ability to send Git diff review comments to the active agent thread.
```

## Recording checklist

1. Local unstaged diff: drag-select → comment → expand list → Edit
2. Toolbar `Send to Agent (N)` → agent card (not a JSON wall)
3. Click path / jump back to hunk
4. Keyboard-only pass for add → submit → send
5. Light and Dark still frames
