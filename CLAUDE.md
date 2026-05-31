# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

All commands run from `reports-app/`:

```bash
# Dev (compiles Rust + starts Vite, opens app window)
npm run tauri dev

# Release build (produces installer in src-tauri/target/release/bundle/)
npm run tauri build

# Frontend only
npm run dev

# Rust tests only
cd src-tauri && cargo test
```

**Release workflow:** commit + push to `main`, then `git tag vX.Y.Z && git push origin vX.Y.Z` to trigger GitHub Actions build. **Do not push a tag before testing locally.**

## Architecture

This is a **Tauri 2 desktop app** (Windows) that connects to on-premise SQL Server ERP databases and provides an AI agent interface.

```
reports-app/
├── src/                        # React + TypeScript frontend
│   ├── App.tsx                 # Root: login gate → 5-page nav (reports/saved/ai/addons/settings)
│   └── components/ui/
│       ├── ai-assistant-interface.tsx   # Main AI chat UI
│       ├── generic-report-page.tsx      # SQL → table/PDF/Excel reports
│       ├── settings-page.tsx            # Secrets, updater, themes, OTA sync status
│       └── sql-login-page.tsx           # SQL Server connection form
└── src-tauri/src/              # Rust backend
    ├── lib.rs                  # All Tauri commands + AppState + SQL connection logic
    ├── ai_agent.rs             # AI agent loop (OpenRouter API, tool dispatch, Telegram bot)
    ├── agent_tools.rs          # Tool implementations: SQL execution, export, patterns, favorites
    ├── agent_content_sync.rs   # OTA sync: Supabase → disk cache → memory
    ├── agent_error_log.rs      # Silent background error logging to Supabase
    ├── agent_memory.rs         # Vector-based memory (Supabase pgvector)
    ├── erp_profile.rs          # ERP type detection + agent prompt loading
    ├── pattern_catalog.rs      # Static catalog of named SQL patterns (pattern_id → section slug)
    ├── infinity_inventory_sql.rs # Advanced SQL for InfinityRetailDB (OTA + embedded sql-split/)
    ├── gotenberg.rs            # HTML→PDF via Gotenberg (Basic Auth, multipart POST)
    ├── pdf_generator.rs        # printpdf-based PDF (fallback for export_last_result)
    ├── excel_generator.rs      # rust_xlsxwriter Excel export
    ├── supabase_config.rs      # Supabase URL/keys + secrets fetch/save via RPC
    ├── erp_adapters.rs         # ERP-specific SQL adapters (product search, POS)
    ├── telegram.rs             # Telegram Bot API client
    ├── scheduler.rs            # Scheduled report delivery (SQLite)
    ├── pharmacy_share.rs       # Pharmacy product sharing via Supabase
    ├── pos_sale.rs             # POS sale processing
    └── sql-split/              # Advanced SQL files (no BOM — SQL Server rejects BOM)
        01-purchase-order.sql .. 07-product-movement.sql
```

## Key Flows

### ERP Detection
On connection, `erp_profile::detect_erp_kind()` probes SQL Server schema: checks for `Inventory.Data_Products` (→ InfinityRetailDB) or `dbo.ITEMS` (→ Marketing2026). Result stored in `AppState.erp_kind`.

### Agent System Prompt & Patterns — OTA Priority
`erp_profile::load_agent_patterns(erp)` loads in this order:
1. **Memory cache** (`agent_content_sync` static `RwLock<HashMap>`)
2. **Disk cache** (`AppData/.../agent_cloud_cache/{bundle_key}.json`)
3. **Supabase** via `get_agent_bundle` RPC (auth: token hash in `app_access` table)
4. **Embedded binary** (`include_str!("../../AGENT_*.md")`) — compile-time fallback

Sync runs every 15 min via `refresh_agent_cloud_content`. **The AGENT_*.md files must exist at compile time** (they're `include_str!`-embedded), but at runtime the Supabase version takes precedence.

### Agent Tool Dispatch
`ai_agent.rs` runs an agentic loop calling OpenRouter (model: `minimax/minimax-m2.7`). Tools split into:
- **Inline tools** (handled directly in loop): `execute_raw_sql`, `explore_local_schema`, `search_schema`
- **Extended tools** (dispatched via `agent_tools::dispatch_extended_tool`): `run_query_pattern`, `export_last_result`, `export_html_pdf`, `save_favorite_query`, `validate_sql`, etc.

`run_query_pattern` resolves a pattern name → `## PATTERN:` section in the agent MD file → extracts SQL → runs it. For InfinityRetailDB BATCH patterns, delegates to `infinity_inventory_sql::sql_for_slug()` which checks Supabase cache then `sql-split/*.sql` embedded files.

### PDF Generation — Two Paths
1. **`export_html_pdf`** (new): agent generates full HTML+CSS → sent to Gotenberg at `http://187.127.111.243:32768` (Basic Auth: `admin:Flashdb@3200`) → PDF bytes returned
2. **`export_last_result` with format=pdf** (legacy): uses `pdf_generator.rs` (printpdf + manual Arabic BiDi reshaping) — kept as fallback

### Supabase Tables
- `app_access` — token auth (SHA-256 hash). All sensitive RPCs verify against this.
- `agent_content_bundles` — system prompts (`infinity_agent_md`, `marketing_agent_md`)
- `agent_pattern_sql` — SQL patterns per ERP kind (RLS-protected, accessed via `get_agent_pattern` RPC)
- `app_secrets` — OpenRouter/OpenAI API keys per token
- `agent_errors` — silent error log from `agent_error_log::log_error_background()`
- `agent_memory` — pgvector embeddings for agent memory recall

### Secret Storage
Secrets (OpenRouter key, Telegram token) stored encrypted in `tauri-plugin-store` (`settings.json`). On startup, `resolve_app_secrets()` checks local store first, then fetches from Supabase if empty. AES-256-GCM encryption, key derived in `lib.rs`.

## Important Constraints

- **SQL Server dialect only** — all queries are T-SQL (`SELECT/WITH`, `TOP N`, `GETDATE()`). Never use PostgreSQL syntax in agent-generated SQL.
- **`sql-split/*.sql` must have NO BOM** — SQL Server rejects UTF-8 BOM (`\xEF\xBB\xBF`) and throws `Token error: Incorrect syntax near ';'`.
- **Agent MD files must exist at compile time** — `include_str!` in `erp_profile.rs` and `infinity_inventory_sql.rs` will fail the build if files are missing.
- **`[FILE_PATH:...]` protocol** — tool results embed this marker; `ai-assistant-interface.tsx` parses it to show the download button (only on the last message containing a file).
- **`pattern_call_count` cap** = 5 per turn. After that, agent gets a message to summarize — it must NOT tell the user the tool is "disabled".
- **Max SQL per turn** = 5 (`MAX_SQL_PER_TURN`). Same cap applies to `execute_raw_sql`.

## GitHub Actions
`.github/workflows/release.yml` builds on `windows-latest` and publishes a GitHub Release on any `v*` tag push. The workflow extracts changelog from `CHANGELOG.md` matching the tag version and posts it as the release body. The updater endpoint is `github.com/Hadani0mar/al-tabi/releases/latest/download/latest.json`.

Signing keys: `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty) in repo secrets.
