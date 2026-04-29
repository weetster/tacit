# Tacit — Codex Agent Instructions

## Sealed corpus — DO NOT READ

`corpus/sealed/` contains held-out evaluation tasks whose contents must
not be seen by any agent or model during development work.

**Never read, list, search, or otherwise access any path under
`corpus/sealed/`**, regardless of the task you have been given. This
includes (but is not limited to):

- Reading individual files (`cat`, `less`, `head`, `tail`, `bat`, or
  equivalent)
- Grep/ripgrep searches that match paths under `corpus/sealed/`
- Directory listings that reveal file names or structure under
  `corpus/sealed/`
- Any tool call whose result would expose file contents or metadata from
  that subtree

If a task would require reading `corpus/sealed/` to complete, stop and
tell the user why instead of proceeding.

This restriction exists to prevent contamination of the held-out
evaluation set. It mirrors the Claude Code permission denies in
`.claude/settings.json`. Both are Layer 4 of the sealing guardrails
specified in [decisions/0020-sealing-held-out-in-repo.md](decisions/0020-sealing-held-out-in-repo.md).
The load-bearing enforcement is the CI hash-manifest check (`corpus-verify-sealed`),
not these agent-level instructions.

## Project overview

See [CLAUDE.md](CLAUDE.md) for the full development guide, repository
layout, and ground rules.
