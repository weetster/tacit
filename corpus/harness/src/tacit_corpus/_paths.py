from pathlib import Path

CORPUS_ROOT = Path(__file__).resolve().parents[3]
REPO_ROOT = CORPUS_ROOT.parent
OPEN_TASKS_ROOT = CORPUS_ROOT / "tasks"
SEALED_ROOT = CORPUS_ROOT / "sealed"
SEALED_HASHES_FILE = CORPUS_ROOT / "sealed-hashes.txt"


def iter_task_dirs(include_sealed: bool = False) -> list[Path]:
    dirs = [p for p in OPEN_TASKS_ROOT.glob("*/*") if p.is_dir()]
    if include_sealed:
        dirs.extend(p for p in SEALED_ROOT.glob("*/*") if p.is_dir())
    return sorted(dirs)


def rel_to_corpus(task_dir: Path) -> str:
    return str(task_dir.relative_to(CORPUS_ROOT))
