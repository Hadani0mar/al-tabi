# Agent and Reporting Changelog

## 2026-06-03

This documentation was added after the code update was pushed to GitHub, per the requested order.

Published code commit:

```text
cf1be45
```

### Purpose

Improve report generation, Telegram behavior, receipt/report printing, and token usage by reducing unnecessary model calls and repeated tool loops.

### Main Changes

- Added a direct desktop fast path for known query patterns before calling the model.
- Added a direct Telegram fast path for known query patterns before calling the model.
- Saved every generated query result with a visible report ID.
- Allowed Telegram users to request a saved report by ID.
- Reduced repeated tool loops around:

```text
run_query_pattern
execute_raw_sql
search_schema
```

- Removed PDF generation as a model tool dependency for normal report printing.
- Kept full report rows in application state and used the UI or Telegram report ID flow for full output.
- Added Telegram chat actions:

```text
typing
upload_document
```

- Added clear Telegram handling for non-text messages, files, and photos.
- Updated expiry report ordering so the current year appears first, then older years below it.
- Fixed expiry day calculations to compare dates only, avoiding time-of-day skew.
- Changed Telegram and desktop summaries to emphasize the report type/title instead of showing row count as the primary line.
- Updated Gotenberg/HTML report output so reports render with application identity and printable table output.
- Improved receipt printing and local receipt archive support.

### Key Files

```text
src-tauri/src/ai_agent.rs
src-tauri/src/agent_tools.rs
src-tauri/src/telegram.rs
src-tauri/src/lib.rs
src-tauri/src/gotenberg.rs
src/components/ui/ai-assistant-interface.tsx
AGENT_Marketing2026.md
AGENT_InfinityRetailDB.md
QUERY_PATTERNS.md
```

### Verification

Commands run before publishing:

```powershell
npm run build
cargo check
```

Both completed successfully.

Known unrelated warning:

```text
sanitize_response_file_paths is never used
```
