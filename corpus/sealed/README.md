# corpus/sealed — Phase 3 held-out tasks

**Do not read these files during primer, training, or evaluation-setup
work.** They are the held-out subset of the Phase 3 evaluation corpus.
Tampering or contamination corrupts the Phase 3 exit criterion.

## How sealing works here

Per [ADR 0020](../../decisions/0020-sealing-held-out-in-repo.md), the
held-out subset is sealed in-repo via four layers:

1. This directory, separate from `corpus/tasks/`.
2. `corpus/sealed-hashes.txt` — BLAKE3 per file, CI-verified on every push.
3. Harness default excludes this tree (`--include-sealed` opts in).
4. Claude Code permission denies on `corpus/sealed/**`.

## If you need to edit a sealed task

Don't — except through an ADR. The sealing rules post-freeze require:

1. Propose an ADR amending or superseding [ADR 0020](../../decisions/0020-sealing-held-out-in-repo.md).
2. Make the edit.
3. Run `uv run corpus-verify-sealed --write` to regenerate `sealed-hashes.txt`.
4. Open a PR with the ADR, the sealed edit, and the manifest regen all
   together. Reviewers see the full picture.

## If you are writing primer or training material

Walk `corpus/tasks/` only. Do not set `--include-sealed` on any harness
command whose output is going to inform primer content.
