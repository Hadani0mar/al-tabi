import { lazy, Suspense, useState, useEffect, useCallback } from "react";
import { SqlLoginPage } from "@/components/ui/sql-login-page";
import MihbarNav from "@/components/ui/futuristic-nav";
import { AppShellHeader } from "@/components/ui/app-shell-header";
import { FIXED_AI_MODEL } from "@/lib/ai-config";
import { applyTheme, loadActiveTheme } from "@/lib/themes";
import { invoke } from "@tauri-apps/api/core";
import { load } from "@tauri-apps/plugin-store";
import { check } from "@tauri-apps/plugin-updater";
import { X, ArrowUpCircle, Bot } from "lucide-react";

const SchedulerPage = lazy(() =>
  import("@/components/ui/scheduler-page").then((m) => ({ default: m.SchedulerPage })),
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
const AddonsPage = lazy(() =>
  import("@/components/ui/addons-page").then((m) => ({ default: m.AddonsPage })),
);

export interface ConnectionInfo {
  server: string;
  port: number;
  database: string;
  username: string;
  password: string;
  use_windows_auth: boolean;
  disable_encryption: boolean;
  server_version: string | null;
}

const STORE_FILE = "connections.dat";

type Page = "reports" | "saved" | "ai" | "addons" | "settings";
const PAGES: Page[] = ["reports", "saved", "ai", "addons", "settings"];

/** الذكاء في المنتصف — index 2 */
const DEFAULT_PAGE_INDEX = 2;

export default function App() {
  const [connected, setConnected] = useState(false);
  const [connInfo, setConnInfo] = useState<ConnectionInfo | null>(null);
  const [pageIndex, setPageIndex] = useState(DEFAULT_PAGE_INDEX);
  const [groqKey, setGroqKey] = useState("");
  const [autoConnecting, setAutoConnecting] = useState(false);
  const [visited, setVisited] = useState({ ai: true, saved: false, addons: false, settings: false });
  const aiModel = FIXED_AI_MODEL;

  // تنبيهات التحديث
  interface UpdateAlert { id: string; type: "app" | "agent"; message: string; version?: string; }
  const [updateAlerts, setUpdateAlerts] = useState<UpdateAlert[]>([]);
  const dismissAlert = useCallback((id: string) => setUpdateAlerts(a => a.filter(x => x.id !== id)), []);

  // فحص التحديثات عند الاتصال
  useEffect(() => {
    if (!connected) return;
    const timer = setTimeout(async () => {
      const alerts: UpdateAlert[] = [];
      // 1) فحص تحديث التطبيق
      try {
        const update = await check();
        if (update) {
          alerts.push({ id: "app", type: "app", message: `يتوفر إصدار جديد للتطبيق`, version: update.version });
        }
      } catch { /* صامت */ }
      // 2) فحص تحديث الوكيل
      try {
        const status = await invoke<{ bundles_updated: number; patterns_updated: number; error?: string }>(
          "refresh_agent_cloud_content", { force: false }
        );
        const total = (status.bundles_updated ?? 0) + (status.patterns_updated ?? 0);
        if (total > 0) {
          alerts.push({ id: "agent", type: "agent", message: `تم تحديث تعليمات الوكيل الذكي (${total} تغيير)` });
        }
      } catch { /* صامت */ }
      if (alerts.length > 0) setUpdateAlerts(alerts);
    }, 4000); // بعد 4 ثواني من الاتصال
    return () => clearTimeout(timer);
  }, [connected]);

  useEffect(() => {
    loadActiveTheme().then(applyTheme).catch(console.error);
  }, []);

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
          disable_encryption: enc.disable_encryption === "true",
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
          setPageIndex(DEFAULT_PAGE_INDEX);
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

  const currentPage = PAGES[pageIndex] ?? "ai";

  useEffect(() => {
    if (currentPage === "saved") setVisited((v) => (v.saved ? v : { ...v, saved: true }));
    if (currentPage === "addons") setVisited((v) => (v.addons ? v : { ...v, addons: true }));
    if (currentPage === "settings") setVisited((v) => (v.settings ? v : { ...v, settings: true }));
  }, [currentPage]);

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
    setPageIndex(DEFAULT_PAGE_INDEX);
    setGroqKey("");
    setVisited({ ai: true, saved: false, addons: false, settings: false });
  }

  function handleConnected(info: ConnectionInfo) {
    setConnInfo(info);
    setConnected(true);
    setPageIndex(DEFAULT_PAGE_INDEX);
    setVisited({ ai: true, saved: false, addons: false, settings: false });
  }

  if (!connected) {
    return (
      <SqlLoginPage
        autoConnecting={autoConnecting}
        onConnected={handleConnected}
      />
    );
  }

  return (
    <div className="flex min-h-screen flex-col" dir="rtl" style={{ background: "var(--bg-canvas)" }}>
      <AppShellHeader businessName={connInfo?.database} connected={connected} />

      {/* تنبيهات التحديث */}
      {updateAlerts.length > 0 && (
        <div className="fixed top-16 left-0 right-0 z-50 flex flex-col gap-2 px-4 pt-2 pointer-events-none">
          {updateAlerts.map(alert => (
            <div
              key={alert.id}
              className="pointer-events-auto flex items-center gap-3 rounded-xl border px-4 py-3 text-sm shadow-lg backdrop-blur-md animate-in slide-in-from-top-2 duration-300"
              style={{
                background: alert.type === "app"
                  ? "color-mix(in srgb, var(--bg-canvas) 85%, #10b981)"
                  : "color-mix(in srgb, var(--bg-canvas) 85%, #7c3aed)",
                borderColor: alert.type === "app" ? "rgba(16,185,129,0.3)" : "rgba(124,58,237,0.3)",
                color: "var(--fg-1)",
              }}
            >
              {alert.type === "app"
                ? <ArrowUpCircle className="w-4 h-4 shrink-0" style={{ color: "#10b981" }} />
                : <Bot className="w-4 h-4 shrink-0" style={{ color: "#7c3aed" }} />}
              <span className="flex-1 font-medium">
                {alert.message}
                {alert.version && <span className="mr-1 font-bold" dir="ltr">v{alert.version}</span>}
                {alert.type === "app" && (
                  <span className="text-xs opacity-70 block mt-0.5">اذهب للإعدادات ← التحديثات لتثبيته</span>
                )}
              </span>
              <button
                onClick={() => dismissAlert(alert.id)}
                className="rounded-lg p-1 transition-colors hover:opacity-70"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          ))}
        </div>
      )}
      <main className="flex-1 pb-28">
        <Suspense fallback={null}>
          {currentPage === "reports" && (
            <SchedulerPage connInfo={connInfo} />
          )}
          {visited.ai && (
            <div className={currentPage === "ai" ? "contents" : "hidden"} aria-hidden={currentPage !== "ai"}>
              <AIAssistantInterface groqKey={groqKey} aiModel={aiModel} />
            </div>
          )}
          {visited.saved && currentPage === "saved" && <SavedQueriesPage />}
          {visited.addons && currentPage === "addons" && <AddonsPage />}
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
