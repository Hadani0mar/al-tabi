//! مزامنة برومبت الوكيل والأنماط من Supabase — بدون تحديث التطبيق.
//! الأولوية: ذاكرة → قرص (AppData) → Supabase → مضمّن في التطبيق.

use crate::erp_profile::ErpKind;
use crate::supabase_config::{SUPABASE_ANON_KEY, SUPABASE_URL};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const REFRESH_INTERVAL_SECS: u64 = 900; // 15 دقيقة
const MANIFEST_RPC: &str = "get_agent_sync_manifest";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedBundle {
    content: String,
    version: i32,
    sha256: String,
    fetched_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedPatternSql {
    sql: String,
    version: i32,
    sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentSyncStatus {
    pub last_check_unix: u64,
    pub last_success_unix: u64,
    pub bundles_updated: u32,
    pub patterns_updated: u32,
    pub source: String,
    pub error: Option<String>,
}

static MEMORY_BUNDLES: OnceLock<RwLock<HashMap<String, CachedBundle>>> = OnceLock::new();
static MEMORY_PATTERNS: OnceLock<RwLock<HashMap<String, CachedPatternSql>>> = OnceLock::new();
static SYNC_STATUS: OnceLock<RwLock<AgentSyncStatus>> = OnceLock::new();

fn memory_bundles() -> &'static RwLock<HashMap<String, CachedBundle>> {
    MEMORY_BUNDLES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn memory_patterns() -> &'static RwLock<HashMap<String, CachedPatternSql>> {
    MEMORY_PATTERNS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn sync_status() -> &'static RwLock<AgentSyncStatus> {
    SYNC_STATUS.get_or_init(|| RwLock::new(AgentSyncStatus::default()))
}

pub fn cache_root() -> PathBuf {
    crate::agent_tools::app_data_dir().join("agent_cloud_cache")
}

fn bundle_cache_path(bundle_key: &str) -> PathBuf {
    cache_root().join(format!("{bundle_key}.json"))
}

fn pattern_cache_path(erp: ErpKind, slug: &str) -> PathBuf {
    cache_root()
        .join("patterns")
        .join(erp.kind_id())
        .join(format!("{slug}.json"))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

pub fn bundle_key_for_erp(erp: ErpKind) -> &'static str {
    match erp {
        ErpKind::InfinityRetailDb => "infinity_agent_md",
        ErpKind::Marketing2026 | ErpKind::Unknown => "marketing_agent_md",
    }
}

fn http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap()
}

fn read_disk_bundle(bundle_key: &str) -> Option<CachedBundle> {
    let path = bundle_cache_path(bundle_key);
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_disk_bundle(bundle_key: &str, bundle: &CachedBundle) -> std::io::Result<()> {
    let dir = cache_root();
    std::fs::create_dir_all(&dir)?;
    std::fs::write(bundle_cache_path(bundle_key), serde_json::to_string(bundle).unwrap_or_default())
}

fn read_disk_pattern(erp: ErpKind, slug: &str) -> Option<CachedPatternSql> {
    let raw = std::fs::read_to_string(pattern_cache_path(erp, slug)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_disk_pattern(erp: ErpKind, slug: &str, pat: &CachedPatternSql) -> std::io::Result<()> {
    let path = pattern_cache_path(erp, slug);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string(pat).unwrap_or_default())
}

fn store_bundle(bundle_key: &str, content: &str, version: i32) {
    let cached = CachedBundle {
        content: content.to_string(),
        version,
        sha256: sha256_hex(content),
        fetched_at_unix: now_unix(),
    };
    if let Ok(mut m) = memory_bundles().write() {
        m.insert(bundle_key.to_string(), cached.clone());
    }
    let _ = write_disk_bundle(bundle_key, &cached);
}

fn store_pattern(erp: ErpKind, slug: &str, sql: &str, version: i32) {
    let key = format!("{}:{}", erp.kind_id(), slug);
    let cached = CachedPatternSql {
        sql: sql.to_string(),
        version,
        sha256: sha256_hex(sql),
    };
    if let Ok(mut m) = memory_patterns().write() {
        m.insert(key, cached.clone());
    }
    let _ = write_disk_pattern(erp, slug, &cached);
}

/// يحمّل محتوى AGENT من الذاكرة أو القرص (بعد مزامنة سابقة)
pub fn load_cached_agent_md(erp: ErpKind) -> Option<String> {
    let key = bundle_key_for_erp(erp);
    if let Ok(guard) = memory_bundles().read() {
        if let Some(b) = guard.get(key) {
            if b.content.contains("## PATTERN:") {
                return Some(b.content.clone());
            }
        }
    }
    read_disk_bundle(key).and_then(|b| {
        if b.content.contains("## PATTERN:") {
            if let Ok(mut guard) = memory_bundles().write() {
                guard.insert(key.to_string(), b.clone());
            }
            Some(b.content)
        } else {
            None
        }
    })
}

pub fn load_cached_pattern_sql(erp: ErpKind, slug: &str) -> Option<String> {
    let key = format!("{}:{}", erp.kind_id(), slug);
    if let Ok(guard) = memory_patterns().read() {
        if let Some(p) = guard.get(&key) {
            return Some(p.sql.clone());
        }
    }
    read_disk_pattern(erp, slug).map(|p| {
        if let Ok(mut guard) = memory_patterns().write() {
            guard.insert(key, p.clone());
        }
        p.sql
    })
}

pub fn get_sync_status() -> AgentSyncStatus {
    sync_status()
        .read()
        .map(|s| s.clone())
        .unwrap_or_default()
}

fn should_refresh(force: bool) -> bool {
    if force {
        return true;
    }
    let status = get_sync_status();
    if status.last_check_unix == 0 {
        return true;
    }
    now_unix().saturating_sub(status.last_check_unix) >= REFRESH_INTERVAL_SECS
}

async fn fetch_manifest() -> Result<Value, String> {
    let client = http_client();
    let url = format!("{}/rest/v1/rpc/{}", SUPABASE_URL, MANIFEST_RPC);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .header("Content-Type", "application/json")
        .json(&json!({}))
        .send()
        .await
        .map_err(|e| format!("Supabase manifest: {e}"))?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(format!("manifest HTTP {status}: {body}"));
    }
    res.json::<Value>().await.map_err(|e| e.to_string())
}

async fn fetch_bundle_row(bundle_key: &str) -> Result<(String, i32), String> {
    let client = http_client();
    let url = format!("{}/rest/v1/rpc/get_agent_bundle", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .header("Content-Type", "application/json")
        .json(&json!({
            "p_bundle_key": bundle_key,
            "p_token": crate::supabase_config::DEFAULT_APP_ACCESS_TOKEN
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(res.text().await.unwrap_or_default());
    }
    let rows: Vec<Value> = res.json().await.map_err(|e| e.to_string())?;
    let row = rows.first().ok_or_else(|| "لا يوجد bundle على Supabase".to_string())?;
    let content = row
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let version = row.get("version").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    if content.is_empty() {
        return Err("محتوى bundle فارغ".to_string());
    }
    Ok((content, version))
}

async fn fetch_pattern_row(erp: ErpKind, slug: &str) -> Result<(String, i32), String> {
    let client = http_client();
    let url = format!("{}/rest/v1/rpc/get_agent_pattern", SUPABASE_URL);
    let res = client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .header("Content-Type", "application/json")
        .json(&json!({
            "p_slug":     slug,
            "p_erp_kind": erp.kind_id(),
            "p_token":    crate::supabase_config::DEFAULT_APP_ACCESS_TOKEN
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(res.text().await.unwrap_or_default());
    }
    let rows: Vec<Value> = res.json().await.map_err(|e| e.to_string())?;
    let row = rows
        .first()
        .ok_or_else(|| format!("لا يوجد pattern {slug}"))?;
    let sql = row
        .get("sql_content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let version = row.get("version").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    Ok((sql, version))
}

fn local_bundle_version(bundle_key: &str) -> i32 {
    read_disk_bundle(bundle_key)
        .map(|b| b.version)
        .unwrap_or(0)
}

fn local_pattern_version(erp: ErpKind, slug: &str) -> i32 {
    read_disk_pattern(erp, slug)
        .map(|p| p.version)
        .unwrap_or(0)
}

/// مزامنة من Supabase — تُستدعى عند بدء التطبيق / قبل الشات / يدوياً
pub async fn refresh_from_supabase(force: bool) -> AgentSyncStatus {
    let mut status = get_sync_status();
    status.last_check_unix = now_unix();

    if !should_refresh(force) {
        status.source = "cache".to_string();
        if let Ok(mut s) = sync_status().write() {
            *s = status.clone();
        }
        return status;
    }

    match fetch_manifest().await {
        Ok(manifest) => {
            let mut bundles_updated = 0u32;
            let mut patterns_updated = 0u32;

            // manifest.bundles = { "bundle_key": version, ... }
            if let Some(obj) = manifest.get("bundles").and_then(|v| v.as_object()) {
                for (key, ver_val) in obj {
                    let remote_ver = ver_val.as_i64().unwrap_or(0) as i32;
                    if remote_ver <= local_bundle_version(key) && !force {
                        continue;
                    }
                    match fetch_bundle_row(key).await {
                        Ok((content, version)) if content.contains("## PATTERN:") => {
                            store_bundle(key, &content, version);
                            bundles_updated += 1;
                        }
                        Ok(_) => eprintln!("[agent_sync] bundle {key} missing ## PATTERN:"),
                        Err(e) => eprintln!("[agent_sync] bundle {key}: {e}"),
                    }
                }
            }

            // manifest.patterns = { "slug:erp_kind": version, ... }
            if let Some(obj) = manifest.get("patterns").and_then(|v| v.as_object()) {
                for (composite_key, ver_val) in obj {
                    let remote_ver = ver_val.as_i64().unwrap_or(0) as i32;
                    // composite_key = "slug:erp_kind"
                    let mut parts = composite_key.splitn(2, ':');
                    let Some(slug) = parts.next() else { continue };
                    let erp = match parts.next() {
                        Some("infinity_retail_db") => ErpKind::InfinityRetailDb,
                        Some("marketing2026") => ErpKind::Marketing2026,
                        _ => continue,
                    };
                    if remote_ver <= local_pattern_version(erp, slug) && !force {
                        continue;
                    }
                    match fetch_pattern_row(erp, slug).await {
                        Ok((sql, version)) if !sql.trim().is_empty() => {
                            store_pattern(erp, slug, &sql, version);
                            patterns_updated += 1;
                        }
                        Ok(_) => {}
                        Err(e) => eprintln!("[agent_sync] pattern {slug}: {e}"),
                    }
                }
            }

            status.last_success_unix = now_unix();
            status.bundles_updated = bundles_updated;
            status.patterns_updated = patterns_updated;
            status.source = if bundles_updated + patterns_updated > 0 {
                "supabase".to_string()
            } else {
                "cache".to_string()
            };
            status.error = None;
        }
        Err(e) => {
            status.error = Some(e);
            status.source = "cache_fallback".to_string();
        }
    }

    if let Ok(mut s) = sync_status().write() {
        *s = status.clone();
    }
    status
}

/// تحميل القرص إلى الذاكرة عند بدء التطبيق (بدون شبكة)
pub fn warm_cache_from_disk() {
    for key in ["marketing_agent_md", "infinity_agent_md"] {
        if let Some(b) = read_disk_bundle(key) {
            if let Ok(mut m) = memory_bundles().write() {
                m.insert(key.to_string(), b);
            }
        }
    }
    let patterns_dir = cache_root().join("patterns");
    if patterns_dir.is_dir() {
        walk_pattern_cache(&patterns_dir);
    }
}

fn walk_pattern_cache(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_pattern_cache(&path);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(pat) = serde_json::from_str::<CachedPatternSql>(&raw) else {
            continue;
        };
        if let Some(slug) = path.file_stem().and_then(|s| s.to_str()) {
            if let Some(erp_id) = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str())
            {
                let key = format!("{erp_id}:{slug}");
                if let Ok(mut m) = memory_patterns().write() {
                    m.insert(key, pat);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_keys_are_stable() {
        assert_eq!(bundle_key_for_erp(ErpKind::InfinityRetailDb), "infinity_agent_md");
        assert_eq!(bundle_key_for_erp(ErpKind::Marketing2026), "marketing_agent_md");
    }
}
