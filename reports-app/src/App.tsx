import { lazy, Suspense, useState, useEffect } from "react";
import { SqlLoginPage } from "@/components/ui/sql-login-page";
import MihbarNav from "@/components/ui/futuristic-nav";
import { AppShellHeader } from "@/components/ui/app-shell-header";
import { FIXED_AI_MODEL } from "@/lib/ai-config";
import { applyTheme, loadActiveTheme } from "@/lib/themes";
import { invoke } from "@tauri-apps/api/core";
import { load } from "@tauri-apps/plugin-store";

const SchedulerPage = lazy(() =>
  import("@/components/ui/scheduler-page").then((m) => ({ default: m.SchedulerPage })),
);
const GenericReportPage = lazy(() =>
  import("@/components/ui/generic-report-page").then((m) => ({ default: m.GenericReportPage })),
);
const AIAssistantInterface = lazy(() =>
  import("@/components/ui/ai-assistant-interface").then((m) => ({ default: m.AIAssistantInterface })),
);
const SavedQueriesPage = lazy(() =>
  import("@/components/ui/saved-queries-page").then((m) => ({ default: m.SavedQueriesPage })),
);
const SettingsPage = lazy(() =>
  import("@/components/ui/settings-page").then((m) => ({ default: m.SettingsPage })),
);

export interface ConnectionInfo {
  server: string;
  port: number;
  database: string;
  username: string;
  password: string;
  use_windows_auth: boolean;
  server_version: string | null;
}

const STORE_FILE = "connections.dat";

type Page = "reports" | "search" | "alerts" | "ai" | "saved" | "settings";
const PAGES: Page[] = ["reports", "search", "alerts", "ai", "saved", "settings"];

export default function App() {
  const [connected, setConnected] = useState(false);
  const [connInfo, setConnInfo] = useState<ConnectionInfo | null>(null);
  const [pageIndex, setPageIndex] = useState(0);
  const [groqKey, setGroqKey] = useState("");
  const [autoConnecting, setAutoConnecting] = useState(false);
  const [visited, setVisited] = useState({ ai: false, saved: false, settings: false });
  const aiModel = FIXED_AI_MODEL;

  useEffect(() => {
    loadActiveTheme().then(applyTheme).catch(console.error);
  }, []);

  // دخول تلقائي في الخلفية — لا نحجب الواجهة
  useEffect(() => {
    let cancelled = false;

    async function tryAutoLogin() {
      try {
        const store = await load(STORE_FILE, { autoSave: false, defaults: {} });
        const autoLoginEnabled = await store.get<boolean>("auto_login");
        const lastConnName = await store.get<string>("last_connection");

        if (!autoLoginEnabled || !lastConnName) return;

        const enc = await store.get<Record<string, string>>(`conn_${lastConnName}`);
        if (!enc) return;

        if (!cancelled) setAutoConnecting(true);

        const password = enc.password
          ? await invoke<string>("decrypt_value", { encrypted: enc.password })
          : "";

        const conn = {
          server: enc.server ?? "",
          port: parseInt(enc.port ?? "1433"),
          database: enc.database ?? "",
          username: enc.username ?? "",
          password,
          use_windows_auth: enc.use_windows_auth === "true",
        };

        const result = await invoke<{ success: boolean; message: string; server_version: string | null }>(
          "test_sql_connection",
          { conn },
        );

        if (cancelled) return;

        if (result.success) {
          await invoke("set_active_connection", { conn }).catch(console.error);
          setConnInfo({ ...conn, server_version: result.server_version });
          setConnected(true);
        }
      } catch (err) {
        console.error("Auto-login failed:", err);
      } finally {
        if (!cancelled) setAutoConnecting(false);
      }
    }

    tryAutoLogin();
    return () => {
      cancelled = true;
    };
  }, []);

  const currentPage = PAGES[pageIndex] ?? "reports";

  useEffect(() => {
    if (currentPage === "ai") setVisited((v) => (v.ai ? v : { ...v, ai: true }));
    if (currentPage === "saved") setVisited((v) => (v.saved ? v : { ...v, saved: true }));
    if (currentPage === "settings") setVisited((v) => (v.settings ? v : { ...v, settings: true }));
  }, [currentPage]);

  // مفتاح AI فقط عند فتح تبويب الوكيل
  useEffect(() => {
    if (!connected || !visited.ai || groqKey) return;
    invoke<{ openrouter_api_key: string }>("load_app_secrets_settings")
      .then((remote) => setGroqKey(remote.openrouter_api_key ?? ""))
      .catch(console.error);
  }, [connected, visited.ai, groqKey]);

  async function handleLogout() {
    try {
      const store = await load(STORE_FILE, { autoSave: false, defaults: {} });
      await store.set("auto_login", false);
      await store.save();
    } catch (err) {
      console.error("Logout cleanup failed:", err);
    }
    setConnInfo(null);
    setConnected(false);
    setPageIndex(0);
    setGroqKey("");
  }

  if (!connected) {
    return (
      <SqlLoginPage
        autoConnecting={autoConnecting}
        onConnected={(info) => {
          setConnInfo(info);
          setConnected(true);
        }}
      />
    );
  }

  return (
    <div className="flex min-h-screen flex-col" dir="rtl" style={{ background: "var(--bg-canvas)" }}>
      <AppShellHeader businessName={connInfo?.database} connected={connected} />
      <main className="flex-1 pb-28">
        <Suspense fallback={null}>
          {(currentPage === "reports" || currentPage === "alerts") && <SchedulerPage />}
          {currentPage === "search" && (
            <div className="flex flex-col h-screen overflow-y-auto pt-6">
              <GenericReportPage
                connInfo={connInfo!}
                reportName="Product Comprehensive Details"
                reportNameAr="البحث عن تفاصيل المنتجات"
              />
            </div>
          )}
          {visited.ai && (
            <div className={currentPage === "ai" ? "contents" : "hidden"} aria-hidden={currentPage !== "ai"}>
              <AIAssistantInterface groqKey={groqKey} aiModel={aiModel} />
            </div>
          )}
          {visited.saved && currentPage === "saved" && <SavedQueriesPage />}
          {visited.settings && (
            <div className={currentPage === "settings" ? "contents" : "hidden"} aria-hidden={currentPage !== "settings"}>
              <div className="flex flex-col min-h-[calc(100vh-7rem)] w-full">
                <SettingsPage connInfo={connInfo} onLogout={handleLogout} />
              </div>
            </div>
          )}
        </Suspense>
      </main>
      <MihbarNav activeIndex={pageIndex} onNavigate={setPageIndex} />
    </div>
  );
}
