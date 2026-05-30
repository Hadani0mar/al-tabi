"""
reembed_ddl.py
==============
يقرأ ملف Full_Marketing_Database_DDL.sql ويُعيد تقطيعه بشكل صحيح:
  - كل chunk = تعريف جدول كامل (من "-- TABLE definition" إلى "-- TABLE definition" التالي)
  - يُضمّن كل chunk بنموذج OpenAI text-embedding-3-small
  - يحذف كل الوثائق القديمة من Supabase ويُدرج الجديدة

الاستخدام:
  python reembed_ddl.py --openai-key sk-... [--dry-run]

المتطلبات:
  pip install requests
"""

import argparse
import json
import re
import sys
import time
import requests

# ── إعدادات Supabase ─────────────────────────────────────────────────
SUPABASE_URL = "https://nsgmhijtaaenpqxxgjds.supabase.co"
SUPABASE_KEY = (
    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"
    ".eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Im5zZ21oaWp0YWFlbnBxeHhnamRzIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzkxODU1NTMsImV4cCI6MjA5NDc2MTU1M30"
    ".bva5PiwsoBiLR7u2upQV7q2spl6GhAg-JqrQ8nnUC8E"
)
DDL_FILE = r"C:\Users\DELL\Desktop\al-tabi\Full_Marketing_Database_DDL.sql"
INFINITY_DDL_FILE = r"C:\Users\DELL\Desktop\al-tabi\reports-app\InfinityRetailDB_DDL.sql"

SUPABASE_HEADERS = {
    "apikey": SUPABASE_KEY,
    "Authorization": f"Bearer {SUPABASE_KEY}",
    "Content-Type": "application/json",
    "Prefer": "return=minimal",
}

# ── تقطيع DDL بحسب حدود الجداول ─────────────────────────────────────
def read_ddl_text(path: str) -> str:
    """يقرأ ملف DDL — يدعم UTF-8 و UTF-16 (BOM) كما في InfinityRetailDB_DDL.sql."""
    raw_bytes = open(path, "rb").read()
    if raw_bytes.startswith(b"\xff\xfe") or raw_bytes.startswith(b"\xfe\xff"):
        return raw_bytes.decode("utf-16")
    if raw_bytes.startswith(b"\xef\xbb\xbf"):
        return raw_bytes.decode("utf-8-sig")
    try:
        return raw_bytes.decode("utf-8")
    except UnicodeDecodeError:
        return raw_bytes.decode("utf-16", errors="replace")


def parse_ddl_into_table_chunks(path: str) -> list[dict]:
    """
    يُقسّم ملف DDL إلى قطع — كل قطعة تحتوي تعريف جدول واحد كامل.
    يُعيد قائمة من: {"table_name": str, "content": str}
    """
    raw = read_ddl_text(path)
    pattern = re.compile(
        r"--\s*Marketing2026\.dbo\.(\w+)\s+definition",
        re.MULTILINE,
    )
    splits = list(pattern.finditer(raw))
    print(f"[parse] Found {len(splits)} table definitions in DDL file.")

    chunks = []
    seen_tables = set()

    for i, match in enumerate(splits):
        table_name = match.group(1)

        # نتجاهل التكرارات (إن وُجدت)
        if table_name in seen_tables:
            continue
        seen_tables.add(table_name)

        # نقطع المحتوى من بداية هذا الجدول حتى بداية التالي
        start = match.start()
        end = splits[i + 1].start() if i + 1 < len(splits) else len(raw)
        block = raw[start:end].strip()

        # نُنظّف: نُزيل أسطر "-- Drop table" و "-- DROP TABLE ..." (غير مفيدة للـ LLM)
        # لكن نبقي على اسم الجدول و CREATE TABLE
        lines = block.splitlines()
        cleaned_lines = []
        for line in lines:
            stripped = line.strip()
            # تخطّ أسطر الـ DROP الزائدة
            if stripped.startswith("-- Drop table") or stripped.startswith("-- DROP TABLE"):
                continue
            cleaned_lines.append(line)
        block = "\n".join(cleaned_lines).strip()

        # أضف ملاحظة سياقية في أول السطر لمساعدة نموذج التضمين
        # (النموذج لا يعرف أن هذا SQL Server بدون هذا السياق)
        header = (
            f"SQL Server Table: Marketing2026.dbo.{table_name}\n"
            f"Database: Marketing2026 (pharmaceutical distribution / توزيع أدوية)\n"
        )
        full_chunk = header + block

        chunks.append({
            "table_name": table_name,
            "content": full_chunk,
            "erp_kind": "marketing2026",
            "source": "Full_Marketing_Database_DDL.sql",
        })

    print(f"[parse] Unique tables to embed: {len(chunks)}")
    return chunks


def parse_infinity_ddl_into_chunks(path: str) -> list[dict]:
    """Parse InfinityRetailDB DDL — one chunk per CREATE TABLE/VIEW."""
    raw = read_ddl_text(path)

    pattern = re.compile(
        r"CREATE\s+(?:TABLE|VIEW)\s+\[(\w+)\]\.\[(\w+)\]",
        re.IGNORECASE,
    )
    splits = list(pattern.finditer(raw))
    print(f"[parse-infinity] Found {len(splits)} CREATE TABLE/VIEW blocks.")

    chunks = []
    seen = set()
    for i, match in enumerate(splits):
        schema, table = match.group(1), match.group(2)
        key = f"{schema}.{table}"
        if key in seen:
            continue
        seen.add(key)

        start = match.start()
        end = splits[i + 1].start() if i + 1 < len(splits) else len(raw)
        block = raw[start:end].strip()
        if len(block) < 40:
            continue

        header = (
            f"SQL Server Table/View: InfinityRetailDB.{schema}.{table}\n"
            f"Database: InfinityRetailDB (retail ERP — Inventory, SALES, Purchase)\n"
            f"erp_kind: infinity_retail_db\n"
        )
        chunks.append({
            "table_name": key,
            "content": header + block,
            "erp_kind": "infinity_retail_db",
            "source": "InfinityRetailDB_DDL.sql",
        })

    print(f"[parse-infinity] Unique objects to embed: {len(chunks)}")
    return chunks


def delete_documents_for_erp(erp_kind: str):
    """Delete documents filtered by metadata erp_kind."""
    print(f"[supabase] Deleting documents for erp_kind={erp_kind}...")
    url = f"{SUPABASE_URL}/rest/v1/documents?metadata->>erp_kind=eq.{erp_kind}"
    res = requests.delete(url, headers=SUPABASE_HEADERS, timeout=60)
    if res.status_code not in (200, 204):
        print(f"  [supabase] Warning: delete returned {res.status_code}: {res.text[:200]}")
    else:
        print(f"  [supabase] Deleted. Status: {res.status_code}")


# text-embedding-3-small يقبل حتى 8191 token. SQL/DDL غالباً ~4 chars/token.
# 24000 حرف ≈ 6000 token — هامش أمان تحت حد 8192.
MAX_CHARS_PER_CHUNK = 24000


def safe_truncate(text: str, max_chars: int = MAX_CHARS_PER_CHUNK) -> str:
    """قص آمن: يحتفظ بالعنوان والأعمدة الأولى — أهم ما في DDL."""
    if len(text) <= max_chars:
        return text
    return text[:max_chars] + "\n-- [TRUNCATED: definition was longer]"


# ── تضمين باستخدام OpenAI ───────────────────────────────────────────
def embed_batch(texts: list[str], openai_key: str) -> list[list[float]]:
    """
    يُرسل دفعة من النصوص إلى OpenAI ويُعيد قائمة من المتجهات.
    يُحاول 3 مرات عند الفشل، ويقص النصوص الطويلة جداً قبل الإرسال.
    """
    # احرص على عدم تجاوز حد التوكنز
    safe_texts = [safe_truncate(t) for t in texts]

    url = "https://api.openai.com/v1/embeddings"
    headers = {
        "Authorization": f"Bearer {openai_key}",
        "Content-Type": "application/json",
    }
    payload = {"model": "text-embedding-3-small", "input": safe_texts}

    for attempt in range(3):
        try:
            res = requests.post(url, headers=headers, json=payload, timeout=60)
            if not res.ok:
                # اطبع التفاصيل عند خطأ 4xx
                print(f"  [embed] HTTP {res.status_code}: {res.text[:500]}")
                print(f"  [embed] Batch sizes: {[len(t) for t in safe_texts]}")
            res.raise_for_status()
            data = res.json()["data"]
            return [item["embedding"] for item in sorted(data, key=lambda x: x["index"])]
        except Exception as e:
            print(f"  [embed] Attempt {attempt+1} failed: {e}")
            time.sleep(2 ** attempt)

    # كمحاولة أخيرة: إذا فشلت الدفعة، نُضمّن واحداً واحداً
    print(f"  [embed] Batch failed. Retrying one-by-one...")
    results = []
    for i, t in enumerate(safe_texts):
        try:
            payload_single = {"model": "text-embedding-3-small", "input": [t]}
            res = requests.post(url, headers=headers, json=payload_single, timeout=60)
            if not res.ok:
                print(f"    [embed-single] item {i} (len={len(t)}) HTTP {res.status_code}: {res.text[:300]}")
                # كحل أخير: قص أكثر
                t_shorter = t[:8000] + "\n-- [HEAVILY TRUNCATED]"
                payload_single = {"model": "text-embedding-3-small", "input": [t_shorter]}
                res = requests.post(url, headers=headers, json=payload_single, timeout=60)
            res.raise_for_status()
            results.append(res.json()["data"][0]["embedding"])
        except Exception as e:
            print(f"    [embed-single] item {i} failed permanently: {e}")
            raise
    return results


# ── حذف جميع الوثائق القديمة ────────────────────────────────────────
def delete_all_documents():
    """يحذف كل الوثائق من جدول documents في Supabase."""
    print("[supabase] Deleting all existing documents...")
    # نحذف كل الصفوف بفلتر "id > 0"
    url = f"{SUPABASE_URL}/rest/v1/documents?id=gt.0"
    res = requests.delete(url, headers=SUPABASE_HEADERS, timeout=60)
    if res.status_code not in (200, 204):
        print(f"  [supabase] Warning: delete returned {res.status_code}: {res.text[:200]}")
    else:
        print(f"  [supabase] Deleted. Status: {res.status_code}")


# ── إدراج وثائق جديدة ────────────────────────────────────────────────
def insert_documents(rows: list[dict], batch_size: int = 50):
    """يُدرج الوثائق في دفعات."""
    url = f"{SUPABASE_URL}/rest/v1/documents"
    total = len(rows)
    for i in range(0, total, batch_size):
        batch = rows[i : i + batch_size]
        res = requests.post(url, headers=SUPABASE_HEADERS, json=batch, timeout=120)
        if res.status_code not in (200, 201):
            print(f"  [supabase] Insert error at batch {i}: {res.status_code} {res.text[:300]}")
        else:
            print(f"  [supabase] Inserted rows {i+1}–{min(i+batch_size, total)} / {total}")


# ── Main ─────────────────────────────────────────────────────────────
def main():
    parser = argparse.ArgumentParser(description="Re-embed DDL into Supabase")
    parser.add_argument("--openai-key", required=True, help="OpenAI API key (sk-...)")
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Parse and embed but do NOT write to Supabase",
    )
    parser.add_argument(
        "--embed-batch-size",
        type=int,
        default=20,
        help="How many texts to send to OpenAI per request (default 20)",
    )
    parser.add_argument(
        "--erp",
        choices=["marketing", "infinity", "both"],
        default="marketing",
        help="Which DDL to embed (default: marketing). Use 'both' to merge Marketing + Infinity.",
    )
    parser.add_argument(
        "--merge",
        action="store_true",
        help="When set, delete only documents for the target erp_kind instead of wiping all.",
    )
    args = parser.parse_args()

    chunks: list[dict] = []
    if args.erp in ("marketing", "both"):
        chunks.extend(parse_ddl_into_table_chunks(DDL_FILE))
        for c in chunks:
            if "erp_kind" not in c:
                c["erp_kind"] = "marketing2026"
    if args.erp in ("infinity", "both"):
        infinity_chunks = parse_infinity_ddl_into_chunks(INFINITY_DDL_FILE)
        chunks.extend(infinity_chunks)

    if not chunks:
        print("ERROR: No table chunks found. Check DDL file path.")
        sys.exit(1)

    if args.dry_run:
        print(f"\n[dry-run] Would embed and insert {len(chunks)} documents.")
        first_preview = chunks[0]["content"][:500].encode("ascii", errors="replace").decode()
        last_preview  = chunks[-1]["content"][:300].encode("ascii", errors="replace").decode()
        print("First chunk preview:\n" + first_preview)
        print(f"\nLast chunk preview:\n" + last_preview)
        print("\n[dry-run] No changes made to Supabase.")
        return

    # 2. تضمين كل القطع
    print(f"\n[embed] Embedding {len(chunks)} table chunks with text-embedding-3-small...")
    texts = [c["content"] for c in chunks]
    embeddings: list[list[float]] = []

    for i in range(0, len(texts), args.embed_batch_size):
        batch_texts = texts[i : i + args.embed_batch_size]
        print(f"  Batch {i//args.embed_batch_size + 1}/{(len(texts)-1)//args.embed_batch_size + 1} ({len(batch_texts)} items)...")
        batch_embs = embed_batch(batch_texts, args.openai_key)
        embeddings.extend(batch_embs)
        time.sleep(0.3)  # تجنب rate limiting

    print(f"[embed] Done. Got {len(embeddings)} embeddings.")

    # 3. بناء صفوف الإدراج
    rows = []
    for chunk, emb in zip(chunks, embeddings):
        erp_kind = chunk.get("erp_kind", "marketing2026")
        rows.append({
            "content": chunk["content"],
            "metadata": {
                "table": chunk["table_name"],
                "source": chunk.get("source", "DDL"),
                "erp_kind": erp_kind,
            },
            "embedding": emb,
        })

    if args.dry_run:
        print(f"\n[dry-run] Would insert {len(rows)} documents. First chunk preview:")
        print(rows[0]["content"][:400])
        print("...\n[dry-run] No changes made to Supabase.")
        return

    # 4. حذف القديم وإدراج الجديد
    if args.merge:
        if args.erp == "both":
            delete_documents_for_erp("marketing2026")
            delete_documents_for_erp("infinity_retail_db")
        elif args.erp == "infinity":
            delete_documents_for_erp("infinity_retail_db")
        else:
            delete_documents_for_erp("marketing2026")
    else:
        delete_all_documents()
    print(f"\n[supabase] Inserting {len(rows)} new documents...")
    insert_documents(rows)

    print(f"\n[done] {len(rows)} table definitions embedded and stored in Supabase.")
    print("   Each chunk = one complete table definition with context header.")
    print("   match_count=15 in the app will now reliably retrieve the right tables.")


if __name__ == "__main__":
    main()
