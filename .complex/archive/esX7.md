## Problem

Claude Code flags Bash tool calls that pipe content (via `cat`, `printf`, heredocs, etc.)
as requiring manual permission approval. This means every `cx` command that sets a body
through stdin triggers a permission prompt, breaking autonomous agent workflows.

Affected patterns:
- `cat <<'BODY' | cx add "title" --body -` — flagged for `cat`
- `printf ... | cx edit <id> --body -` — flagged as piped Bash
- `--body "..."` with markdown headings — flagged due to `#` after newlines

## Solution

Add `--body-file` / `-F` flag to `cx add` and `cx new` (matching `cx edit --file`).

Agent workflow becomes:
1. Write body content using the Write tool (auto-approved, no Bash)
2. `cx add "title" --body-file /tmp/body.md` (simple Bash, auto-approved)

## Acceptance criteria
- `cx add "title" --body-file path.md` creates node with body from file
- `cx new <parent> "title" --body-file path.md` creates child with body from file
- Update CLAUDE.md agent instructions to prefer `--body-file` pattern
