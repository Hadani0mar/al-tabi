# AGENTS.md

This file guides AI coding agents working on this monorepo. The main project is **reports-app** (Tauri 2 desktop app: React 19 + Rust).

---

## Build / Lint / Test

```bash
# ── Frontend (from reports-app/) ──
npm run dev              # Vite dev server (port 1420)
npm run build            # tsc + vite build (catches TS strict errors)

# ── Tauri (desktop app) ──
npm run tauri dev        # Compiles Rust + starts Vite, opens window
npm run tauri build      # Release build → installer in src-tauri/target/

# ── Rust only ──
cd src-tauri && cargo check        # Fast type-check (no codegen)
cd src-tauri && cargo test         # All Rust tests
cd src-tauri && cargo test -- name  # Single test by name substring

# ── Kill previous instance (Windows) ──
taskkill /f /im reports-app.exe
```

**There is no frontend test framework.** TypeScript strict (`tsc --noEmit`) is the frontend CI gate. Rust tests use `#[cfg(test)] mod tests` with `#[test]` in each module.

---

## Code Style — TypeScript / React

### Imports
```
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { motion, AnimatePresence } from "framer-motion";
import { Search, Loader2 } from "lucide-react";
```

- Path alias: `@/` → `src/`
- Group: std lib → Tauri APIs → project modules → third-party UI
- Named exports for components (`export function Foo`), not default exports

### Formatting & Naming
- **Components:** PascalCase, function declarations (`export function Foo()`)
- **Files:** kebab-case (`sql-login-page.tsx`)
- **Interfaces/types:** PascalCase, `Props` / `State` for component-local types
- **Variables/functions:** camelCase
- **CSS:** Tailwind utility classes + CSS variables (`var(--bg-canvas)`, `var(--fg-1)`)
- **RTL:** `dir="rtl"` on every page root

### Conventions
- `"use client"` directive at top of interactive components
- `cn()` from `@/lib/utils` (clsx + tailwind-merge) for className merging
- `Suspense` + `React.lazy()` for code splitting
- `framer-motion` for animations, `lucide-react` for icons
- `style={{ background: "var(--bg-surface)" }}` for dynamic theming (never hardcode colors)
- Comments in Arabic, section separators: `// ─── section ───`

### Error Handling
- `async` functions: try/catch/finally with `console.error` in catch
- `invoke<T>()` calls wrapped in try/catch, string-cast errors: `String(err)`
- `ErrorBoundary` class component wrapping `<App />`
- `useEffect` cleanup via `cancelled` boolean pattern for async race conditions

---

## Code Style — Rust

### Structure
- All commands in `lib.rs`; logic in separate modules (`ai_agent.rs`, `agent_tools.rs`, etc.)
- `AppState` with `Arc<Mutex<...>>` for shared state, `Arc<tokio::sync::Mutex<()>>` for locks
- Arabic doc comments (`//!`) and inline comments
- Section separators: `// ─── section ───`

### Conventions
- `#[derive(Debug, Serialize, Deserialize, Clone)]` on all DTOs
- `serde` + `tiberius` for SQL Server (T-SQL only — no PostgreSQL syntax)
- `tokio` async throughout; `TcpStream` + `tiberius::Client`
- Error handling: `Result<T, String>` or `.map_err(|e| format!(...))`
- Tests: `#[cfg(test)] mod tests { use super::*; #[test] fn ... }` at bottom of file
- `format!()` for error strings, not custom error types

---

## Project Key Facts

- **Two ERP systems:** Marketing2026 (`dbo.*`) and InfinityRetailDB (`Inventory.*`, `SALES.*`)
- **TypeScript strict** (`noUnusedLocals`, `noUnusedParameters`) — fix all errors before commit
- **No .env committed** — `.env.example` for reference only
- **Release:** `git tag vX.Y.Z && git push origin vX.Y.Z` triggers GitHub Actions
- **Secrets (TAURI signing key) never committed** — in .gitignore
- **Reference files:** `ERP_ARCHITECTURE.md` (start here), `AGENT_Marketing2026.md`, `AGENT_InfinityRetailDB.md`
