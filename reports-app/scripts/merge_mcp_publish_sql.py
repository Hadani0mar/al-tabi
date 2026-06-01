#!/usr/bin/env python3
"""Merge tmp_mcp_publish/*.sql into one migration for Supabase MCP."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SQL_DIR = ROOT / "tmp_mcp_publish"
OUT = SQL_DIR / "00_all_agent_seed.sql"

ORDER = [
    "pattern_طلبية-شراء-متقدمة.sql",
    "pattern_أصناف-راكدة-متقدمة.sql",
    "pattern_خطر-الصلاحية-FEFO.sql",
    "pattern_اتجاه-مبيعات-30-30.sql",
    "pattern_أصناف-قيد-التجربة.sql",
    "pattern_أصناف-وهمية.sql",
    "pattern_تصنيف-حركة-الصنف.sql",
    "bundle_infinity_agent_md.sql",
    "bundle_marketing_agent_md.sql",
]


def main() -> None:
    parts: list[str] = ["-- Agent OTA seed (patterns + AGENT bundles)\n"]
    for name in ORDER:
        path = SQL_DIR / name
        if not path.is_file():
            continue
        parts.append(f"\n-- >>> {name}\n")
        parts.append(path.read_text(encoding="utf-8"))
        parts.append("\n")
    OUT.write_text("".join(parts), encoding="utf-8")
    print(f"Wrote {OUT} ({OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
