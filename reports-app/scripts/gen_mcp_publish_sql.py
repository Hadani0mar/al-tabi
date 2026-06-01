#!/usr/bin/env python3
"""Generate SQL files for Supabase MCP execute_sql publishing."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "tmp_mcp_publish"

PATTERNS = [
    ("infinity_retail_db", "طلبية-شراء-متقدمة", "sql-split/01-purchase-order.sql"),
    ("infinity_retail_db", "أصناف-راكدة-متقدمة", "sql-split/02-slow-moving.sql"),
    ("infinity_retail_db", "خطر-الصلاحية-FEFO", "sql-split/03-expiry-risk.sql"),
    ("infinity_retail_db", "اتجاه-مبيعات-30-30", "sql-split/04-sales-trend-30-30.sql"),
    ("infinity_retail_db", "أصناف-قيد-التجربة", "sql-split/05-trial-products.sql"),
    ("infinity_retail_db", "أصناف-وهمية", "sql-split/06-phantom-products.sql"),
    ("infinity_retail_db", "تصنيف-حركة-الصنف", "sql-split/07-product-movement.sql"),
]

BUNDLES = [
    ("marketing_agent_md", "marketing2026", "AGENT_Marketing2026.md"),
    ("infinity_agent_md", "infinity_retail_db", "AGENT_InfinityRetailDB.md"),
]


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def dollar_tag(prefix: str, body: str) -> tuple[str, str]:
    tag = prefix
    i = 0
    while f"${tag}$" in body:
        tag = f"{prefix}{i}"
        i += 1
    return tag, body


def main() -> None:
    OUT.mkdir(exist_ok=True)
    manifest: dict = {"patterns": [], "bundles": []}

    for erp, slug, rel in PATTERNS:
        sql_body = (ROOT / rel).read_text(encoding="utf-8")
        digest = sha256(sql_body)
        tag, _ = dollar_tag(f"pat_{abs(hash(slug)) % 100000}", sql_body)
        sql = f"""INSERT INTO agent_pattern_sql (pattern_slug, erp_kind, sql_content, version, content_sha256, is_active)
VALUES (
  '{slug}',
  '{erp}',
  ${tag}${sql_body}${tag}$,
  1,
  '{digest}',
  true
)
ON CONFLICT (pattern_slug, erp_kind) DO UPDATE SET
  sql_content = EXCLUDED.sql_content,
  version = agent_pattern_sql.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true;"""
        fname = OUT / f"pattern_{slug}.sql"
        fname.write_text(sql, encoding="utf-8")
        manifest["patterns"].append({"file": fname.name, "slug": slug, "bytes": len(sql)})

    for key, erp, rel in BUNDLES:
        path = ROOT / rel
        if not path.is_file():
            continue
        md = path.read_text(encoding="utf-8")
        if "## PATTERN:" not in md:
            continue
        digest = sha256(md)
        tag, _ = dollar_tag(f"bnd_{key}", md)
        sql = f"""INSERT INTO agent_content_bundles (bundle_key, erp_kind, bundle_type, content, version, content_sha256, is_active, changelog)
VALUES (
  '{key}',
  '{erp}',
  'agent_md',
  ${tag}${md}${tag}$,
  1,
  '{digest}',
  true,
  'Published via Supabase MCP'
)
ON CONFLICT (bundle_key) DO UPDATE SET
  content = EXCLUDED.content,
  version = agent_content_bundles.version + 1,
  content_sha256 = EXCLUDED.content_sha256,
  updated_at = now(),
  is_active = true,
  changelog = EXCLUDED.changelog;"""
        fname = OUT / f"bundle_{key}.sql"
        fname.write_text(sql, encoding="utf-8")
        manifest["bundles"].append({"file": fname.name, "key": key, "bytes": len(sql)})

    (OUT / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
