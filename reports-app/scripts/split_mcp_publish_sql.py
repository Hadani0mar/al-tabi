#!/usr/bin/env python3
"""Split merged agent seed SQL into chunks for Supabase MCP apply_migration."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SQL_DIR = ROOT / "tmp_mcp_publish"
MERGED = SQL_DIR / "00_all_agent_seed.sql"
MAX_BYTES = 80_000


def main() -> None:
    if not MERGED.is_file():
        raise SystemExit(f"Run merge_mcp_publish_sql.py first — missing {MERGED}")

    text = MERGED.read_text(encoding="utf-8")
    # split on file markers
    blocks = text.split("\n-- >>> ")
    header = blocks[0]
    chunks: list[str] = []
    current = header
    for block in blocks[1:]:
        piece = "\n-- >>> " + block
        if len(current.encode("utf-8")) + len(piece.encode("utf-8")) > MAX_BYTES and current.strip():
            chunks.append(current)
            current = piece
        else:
            current += piece
    if current.strip():
        chunks.append(current)

    for i, chunk in enumerate(chunks, start=1):
        out = SQL_DIR / f"chunk_{i:02d}.sql"
        out.write_text(chunk, encoding="utf-8")
        print(f"chunk_{i:02d}.sql {out.stat().st_size} bytes")

    print(f"total chunks: {len(chunks)}")


if __name__ == "__main__":
    main()
