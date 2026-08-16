# AGENTS

---

## Agent Tooling

**RTK** is installed for OpenCode and automatically hooks shell commands to compress output. Run intended shell commands normally; or prefix them with `rtk`. Read `.rtk/README.md` and `.rtk/RTK.md` to understand more usage if you confused with it;

**CodeGraph** (`codegraph_*` tools) is used for structural code questions such as definitions, callers, callees, impact, signatures, and focused symbol context. Use normal file search for literal text, comments, strings, logs, docs, and filenames.  Read `.codegraph/README.md` for details.

**Review agent config**: When spawning review sub-agents, treat [`.codex/agents/default.toml`](/home/gaby/Projects/Archive/anchor/.codex/agents/default.toml) as the source of truth for the review-agent defaults. Pass its `model` and `model_reasoning_effort` values through explicitly, do not hard-code alternate review settings, and fail closed if the config is missing or invalid.


## Agent Skills

### Issue tracker

Issues and PRDs are tracked as local markdown files under `.scratch/`; external PRs are not a triage surface. See `docs/agents/issue-tracker.md`.

### Triage labels

This repo uses the canonical triage label vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix`, plus `implemented` for issue slices that are done. See `docs/agents/triage-labels.md`.

### Domain docs

This repo uses a single-context domain-doc layout: root `CONTEXT.md` plus root `docs/adr/`. See `docs/agents/domain.md`.


## Agent Rules

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

### 5. Testing Artifacts

Tests that create files or directories must use a temporary directory or an explicitly isolated test fixture root. Never write test artifacts into source, crate, workspace, or repository directories.

### 6. Module Organization

**Prefer deep modules with a small, stable interface.**

- Split by responsibility and ownership, not by a line-count threshold alone.
- When one module has several cohesive implementation concerns, use a directory module: `name/mod.rs` is the interface/composition point and `name/*.rs` are private implementation modules.
- Keep callers on the existing interface; do not expose internal helpers merely to make a split compile.
- Before extracting, name the seam, preserve its ordering and error invariants, and identify the focused tests that prove them.
- Do not create a module for a one-use wrapper or a speculative future variation; it should improve locality for a real change.
