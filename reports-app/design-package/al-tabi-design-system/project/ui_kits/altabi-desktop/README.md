# al-tabi Desktop — UI Kit

Interactive recreation of the al-tabi desktop application (Tauri/Windows, RTL Arabic).

## What's in here

- `index.html` — entry point. Loads React + Babel and mounts the app. Open in a browser to play with the click-thru.
- `app.jsx` — root, screen routing, persistent state
- `AppShell.jsx` — window chrome, title bar, bottom navigation pill
- `LoginScreen.jsx` — SQL Server connection form (the first thing a user sees)
- `AIChatScreen.jsx` — main "الذكاء" tab — chat with the agent
- `ReportsScreen.jsx` — scheduler with cards + toggles
- `SavedScreen.jsx` — list of saved SQL queries
- `SettingsScreen.jsx` — connection, Telegram, theme, business profile
- `icons.jsx` — Lucide-style SVG icon components

## Interactions

1. App opens on the **Login** screen. The connection form is pre-filled. Click "اتصل بقاعدة البيانات".
2. After a brief "جارٍ الاتصال…" state, you land on the **الذكاء** (AI) tab.
3. Type a question or click one of the suggested prompts → see tool-status → see the assistant response with a table.
4. Use the bottom pill nav to move between tabs.
5. In **التقارير**, toggle a schedule on/off. Click the "+" to see a placeholder.
6. In **الإعدادات**, switch themes (light / dark) to see Mihbar in both modes.

## What's a recreation, what's not

This is a **visual + interaction** recreation. There is no real SQL Server connection, no AI calls, no Telegram. All data is fixture. Use this to:
- Verify the design system in context
- Hand off to engineers as a visual reference
- Build mocks and screenshots for marketing

## Design width

The window is designed at **1180×720**. It scales down inside the design canvas; for inspection it looks best at full size.
