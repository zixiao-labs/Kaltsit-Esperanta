# First-class Plan Mode primitives for agent workflows

## Summary

I'd like to propose making Plan Mode a first-class part of Zed's agent workflow rather than treating planning as plain chat text. The near-term goal is intentionally small: give native and ACP-backed agents a structured way to maintain a visible plan, ask for user approval at the planning/execution boundary, and support cancellation without introducing a long-running autonomous goal system yet.

## Motivation

As agents take on larger coding tasks, users need a lightweight control surface that answers:

- What is the agent about to do?
- Which step is active now?
- What has already completed?
- Where can I approve, redirect, or stop execution?

Zed already has several adjacent pieces: ACP plan updates, elicitation UI, tool authorization, subagents, and thread cancellation. A first-class Plan Mode can connect those pieces without immediately committing to a broader Goal Mode/runtime.

## Proposed MVP

### 1. Structured plan updates

Expose a simple tool/protocol primitive that replaces the current plan with a complete list of entries:

- `content`: human-readable step description
- `status`: `pending`, `in_progress`, or `completed`
- `priority`: `low`, `medium`, or `high`

The client renders the current plan and snapshots completed plans into the thread history.

### 2. Read-only Plan profile/mode

Add a built-in Plan profile that allows investigation and planning tools but excludes edit, write, delete, move, terminal, and other execution-oriented tools by default.

This gives users a safe way to ask for a plan before enabling execution.

### 3. Human approval via elicitation

Use existing elicitation/user-question infrastructure for approval and missing decisions:

- Free-form questions for ambiguity.
- Single-choice questions for branching decisions.
- Multi-choice questions for selecting validation or follow-up steps.

This keeps the planning boundary explicit without inventing a separate approval UI for the MVP.

### 4. Cancellation and status consistency

Cancellation should preserve the latest visible plan and cancel pending elicitation/tool calls. Completed plans can be snapshotted when no pending/in-progress entries remain.

## Non-goals for the MVP

- Persistent long-running goals.
- Durable execution leases or budgets.
- Checkpoints/rollback.
- Cross-session recovery of interrupted provider work.
- A new autonomous scheduler.

Those are important for a future Goal Mode, but they are larger than the initial Plan Mode primitive.

## Open questions

1. Should Plan Mode be modeled as an agent profile, a thread state, or both?
2. Should the protocol grow richer plan statuses such as `failed`, `skipped`, or `cancelled`, or should those remain represented in text for now?
3. Should approval be a generic elicitation request, a specialized plan approval action, or layered on top of tool authorization?
4. How should subagent plans compose into a parent plan without overwhelming the UI?
5. Should completed plan snapshots be searchable/exported as part of thread markdown?

## Why this overlaps with the roadmap

This sits between today's chat/tool loop and a future goal-oriented runtime. It gives users immediate visibility and control for multi-step work while keeping the implementation compatible with existing ACP plan and elicitation concepts.
