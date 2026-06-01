#!/usr/bin/env python3
"""Publish AGENT_*.md + sql-split patterns to Supabase (OTA for all desktop clients).

Usage (from repo root):
  set SUPABASE_SERVICE_ROLE_KEY=eyJ...   # Dashboard → Settings → API → service_role
  python reports-app/scripts/publish_agent_to_supabase.py

Or publish via Supabase MCP execute_sql (INSERT ... ON CONFLICT) from Cursor.

Requires: pip install requests
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path

import requests

SUPABASE_URL = os.environ.get(
    "SUPABASE_URL", "https://nsgmhijtaaenpqxxgjds.supabase.co"
)
SERVICE_KEY = os.environ.get("SUPABASE_SERVICE_ROLE_KEY", "").strip()

ROOT = Path(__file__).resolve().parents[1]  # reports-app/

BUNDLES = [
    {
        "bundle_key": "marketing_agent_md",
        "erp_kind": "marketing2026",
        "path": ROOT / "AGENT_Marketing2026.md",
        "changelog": "Initial / updated marketing agent patterns",
    },
    {
        "bundle_key": "infinity_agent_md",
        "erp_kind": "infinity_retail_db",
        "path": ROOT / "AGENT_InfinityRetailDB.md",
        "changelog": "Infinity agent + inventory batch patterns",
    },
]

PATTERN_FILES = [
    ("infinity_retail_db", "طلبية-شراء-متقدمة", "sql-split/01-purchase-order.sql"),
    ("infinity_retail_db", "أصناف-راكدة-متقدمة", "sql-split/02-slow-moving.sql"),
    ("infinity_retail_db", "خطر-الصلاحية-FEFO", "sql-split/03-expiry-risk.sql"),
    ("infinity_retail_db", "اتجاه-مبيعات-30-30", "sql-split/04-sales-trend-30-30.sql"),
    ("infinity_retail_db", "أصناف-قيد-التجربة", "sql-split/05-trial-products.sql"),
    ("infinity_retail_db", "أصناف-وهمية", "sql-split/06-phantom-products.sql"),
    ("infinity_retail_db", "تصنيف-حركة-الصنف", "sql-split/07-product-movement.sql"),
    ("infinity_retail_db", "فحص-الأصناف-والوحدات", "sql-split/08-check-items-uom.sql"),
    ("infinity_retail_db", "حساب-توفر-المخزون", "sql-split/09-check-availability.sql"),
    ("infinity_retail_db", "المبيعات-وصافي-المطلوب", "sql-split/10-net-required.sql"),
    ("infinity_retail_db", "فواتير-المشتريات-والصلاحية", "sql-split/11-purchase-invoices-expiry.sql"),
]


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def headers() -> dict[str, str]:
    if not SERVICE_KEY:
        print(
            "ERROR: set SUPABASE_SERVICE_ROLE_KEY (service_role — not anon).\n"
            "Dashboard: https://supabase.com/dashboard/project/nsgmhijtaaenpqxxgjds/settings/api",
            file=sys.stderr,
        )
        sys.exit(1)
    return {
        "apikey": SERVICE_KEY,
        "Authorization": f"Bearer {SERVICE_KEY}",
        "Content-Type": "application/json",
        "Prefer": "resolution=merge-duplicates,return=representation",
    }


def upsert_bundle(row: dict) -> None:
    url = f"{SUPABASE_URL}/rest/v1/agent_content_bundles?on_conflict=bundle_key"
    r = requests.post(url, headers=headers(), json=row, timeout=120)
    if r.status_code not in (200, 201):
        print(f"bundle {row['bundle_key']} failed: {r.status_code} {r.text[:500]}", file=sys.stderr)
        r.raise_for_status()
    print(f"✓ bundle {row['bundle_key']} v{row['version']} ({len(row['content'])} chars)")


def upsert_pattern(row: dict) -> None:
    url = f"{SUPABASE_URL}/rest/v1/agent_pattern_sql?on_conflict=pattern_slug,erp_kind"
    r = requests.post(url, headers=headers(), json=row, timeout=120)
    if r.status_code not in (200, 201):
        print(f"pattern {row['pattern_slug']} failed: {r.status_code} {r.text[:500]}", file=sys.stderr)
        r.raise_for_status()
    print(f"✓ pattern {row['pattern_slug']} v{row['version']}")


def next_version(table: str, key_col: str, key_val: str, extra: dict | None = None) -> int:
    """Best-effort version bump by reading current row."""
    params = {key_col: f"eq.{key_val}", "select": "version,content_sha256"}
    if extra:
        for k, v in extra.items():
            params[k] = f"eq.{v}"
    url = f"{SUPABASE_URL}/rest/v1/{table}"
    r = requests.get(url, headers=headers(), params=params, timeout=30)
    if r.status_code != 200 or not r.json():
        return 1
    return int(r.json()[0].get("version", 0)) + 1


def load_env_file() -> None:
    for path in (ROOT / ".env", ROOT.parent / ".env"):
        if not path.is_file():
            continue
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, val = line.partition("=")
            key = key.strip()
            val = val.strip().strip('"').strip("'")
            if key and key not in os.environ:
                os.environ[key] = val


def main() -> None:
    load_env_file()
    print(f"Publishing to {SUPABASE_URL}\n")

    for spec in BUNDLES:
        path: Path = spec["path"]
        if not path.is_file():
            print(f"skip missing {path}", file=sys.stderr)
            continue
        content = path.read_text(encoding="utf-8")
        if "## PATTERN:" not in content:
            print(f"skip {path.name} — no ## PATTERN:", file=sys.stderr)
            continue
        digest = sha256(content)
        ver = next_version("agent_content_bundles", "bundle_key", spec["bundle_key"])
        upsert_bundle(
            {
                "bundle_key": spec["bundle_key"],
                "erp_kind": spec["erp_kind"],
                "bundle_type": "agent_md",
                "content": content,
                "version": ver,
                "content_sha256": digest,
                "is_active": True,
                "changelog": spec["changelog"],
            }
        )

    for erp_kind, slug, rel in PATTERN_FILES:
        path = ROOT / rel
        if not path.is_file():
            print(f"skip missing {path}", file=sys.stderr)
            continue
        sql = path.read_text(encoding="utf-8")
        digest = sha256(sql)
        ver = next_version(
            "agent_pattern_sql",
            "pattern_slug",
            slug,
            {"erp_kind": erp_kind},
        )
        upsert_pattern(
            {
                "pattern_slug": slug,
                "erp_kind": erp_kind,
                "sql_content": sql,
                "version": ver,
                "content_sha256": digest,
                "is_active": True,
            }
        )

    # verify manifest RPC
    r = requests.post(
        f"{SUPABASE_URL}/rest/v1/rpc/get_agent_sync_manifest",
        headers={
            "apikey": os.environ.get("SUPABASE_ANON_KEY", ""),
            "Authorization": f"Bearer {os.environ.get('SUPABASE_ANON_KEY', SERVICE_KEY)}",
            "Content-Type": "application/json",
        },
        json={},
        timeout=30,
    )
    if r.ok:
        m = r.json()
        print("\nManifest:", json.dumps(m, indent=2, ensure_ascii=False)[:800])
    print("\nDone — clients will pull on next startup or AI chat (within ~15 min).")


if __name__ == "__main__":
    main()
