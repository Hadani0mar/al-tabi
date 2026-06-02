# al-tabi — reports-app

تطبيق سطح مكتب (Tauri + React + Rust) للتقارير الذكية فوق ERP محلي على SQL Server.

## أنظمة ERP المدعومة

| ERP | Schema | ملف الوكيل |
|-----|--------|------------|
| **Marketing2026** | `dbo.*` | `AGENT_Marketing2026.md` |
| **InfinityRetailDB** | `Inventory`, `SALES`, `Purchase`, `MyCompany` | `AGENT_InfinityRetailDB.md` |

الاكتشاف تلقائي عند تسجيل الدخول — راجع [`ERP_ARCHITECTURE.md`](./ERP_ARCHITECTURE.md).

## التشغيل

```bash
npm install
npm run tauri dev
```

```bash
cd src-tauri && cargo check
```

## توثيق للوكلاء / المطوّرين

| ابدأ هنا | الغرض |
|----------|--------|
| [`ERP_ARCHITECTURE.md`](./ERP_ARCHITECTURE.md) | معمارية النظامين |
| [`AGENTS.md`](./AGENTS.md) | دليل Cursor / تطوير |
| [`AGENT_FAST_MEMORY.md`](./AGENT_FAST_MEMORY.md) | ذاكرة الوكيل السريعة، جداول Supabase، تقليل التوكنز |
| [`AGENT_Marketing2026.md`](./AGENT_Marketing2026.md) | أنماط SQL Marketing |
| [`AGENT_InfinityRetailDB.md`](./AGENT_InfinityRetailDB.md) | أنماط SQL Infinity |

## IDE

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
