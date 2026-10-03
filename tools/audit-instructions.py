"""Optional documentation measurement; UTF-8 bytes/whitespace words, not model tokens."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path

EXCLUDED = {".git", "target", "vendor", "node_modules", "__pycache__"}


def measure(path: Path) -> dict[str, int]:
    data = path.read_bytes()
    return {"bytes": len(data), "words": len(data.decode("utf-8-sig").split())}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", type=Path,
                        help="Exact files to count; default: documentation inventory of available siblings")
    args = parser.parse_args()
    forge = Path(__file__).resolve().parents[1]
    base = forge.parent
    explicit = bool(args.paths)
    paths = args.paths if explicit else []
    if not explicit:
        for repo in (forge, base / "FFOneClient"):
            if not repo.is_dir():
                continue
            paths.extend(p for p in repo.iterdir() if p.is_file() and p.suffix.lower() == ".md")
            for documentation_root in (repo / "docs", repo / ".agents", repo / "tools"):
                if not documentation_root.is_dir():
                    continue
                for directory, dirs, names in os.walk(documentation_root):
                    dirs[:] = sorted(d for d in dirs if d not in EXCLUDED)
                    paths.extend(Path(directory) / name for name in names
                                 if name.lower().endswith(".md") or name == "openai.yaml")
    files = {}
    for path in sorted({p.resolve() for p in paths}):
        if not path.is_file():
            parser.error(f"File not found: {path}")
        try:
            name = path.relative_to(base).as_posix()
        except ValueError:
            name = str(path)
        try:
            files[name] = measure(path)
        except (OSError, UnicodeError) as exc:
            parser.error(f"Cannot measure {path}: {exc}")
    print(json.dumps({
        "measurement": "UTF-8 bytes and whitespace-separated words; NOT tokens",
        "scope": "explicit files" if explicit else "available documentation, including optional historical evidence",
        "caveat": "Inventory totals are not actual model reads, automatic loading, or task cost. No links are followed.",
        "files": files,
        "total": {key: sum(row[key] for row in files.values()) for key in ("bytes", "words")},
    }, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
