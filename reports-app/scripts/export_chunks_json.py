#!/usr/bin/env python3
"""Apply chunk SQL files via Supabase MCP HTTP bridge (reads files, prints for agent)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SQL_DIR = ROOT / "tmp_mcp_publish"


def main() -> None:
    chunks = sorted(SQL_DIR.glob("chunk_*.sql"))
    if not chunks:
        raise SystemExit("No chunk_*.sql — run gen + merge + split first")
    for p in chunks:
        data = {
            "name": p.stem.replace("-", "_"),
            "path": str(p),
            "bytes": p.stat().st_size,
            "sql": p.read_text(encoding="utf-8"),
        }
        out = SQL_DIR / f"{p.stem}.json"
        out.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
        print(f"wrote {out.name} ({data['bytes']} bytes)")


if __name__ == "__main__":
    main()
