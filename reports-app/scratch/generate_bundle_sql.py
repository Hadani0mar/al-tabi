import hashlib
import os

def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()

def main():
    agent_path = r"c:\Users\DELL\Desktop\al-tabi\reports-app\AGENT_InfinityRetailDB.md"
    output_path = r"c:\Users\DELL\Desktop\al-tabi\reports-app\scratch\update_bundle.sql"
    
    # Create parent dir if not exists
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    
    if not os.path.exists(agent_path):
        print(f"Error: {agent_path} not found")
        return
        
    with open(agent_path, "r", encoding="utf-8") as f:
        content = f.read()
        
    digest = sha256(content)
    
    # We use PostgreSQL dollar quoting ($$content$$) to escape the entire Markdown text safely without worrying about quotes
    sql = f"""-- Update infinity_agent_md bundle in Supabase
INSERT INTO agent_content_bundles (bundle_key, erp_kind, bundle_type, content, version, content_sha256, is_active, changelog)
VALUES (
  'infinity_agent_md',
  'infinity_retail_db',
  'agent_md',
  $$content$${content}$$content$$,
  3, -- Bumping version to 3
  '{digest}',
  true,
  'Updated instructions and triggers for 4 new split patterns: check_items_uom, check_availability, net_required_check, purchase_invoices_expiry.'
)
ON CONFLICT (bundle_key)
DO UPDATE SET
  content = EXCLUDED.content,
  version = EXCLUDED.version,
  content_sha256 = EXCLUDED.content_sha256,
  changelog = EXCLUDED.changelog;
"""

    with open(output_path, "w", encoding="utf-8") as f:
        f.write(sql)
        
    print(f"Successfully generated SQL at {output_path}")
    print(f"SHA-256: {digest}")

if __name__ == "__main__":
    main()
