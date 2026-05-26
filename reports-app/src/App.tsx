import { useState, useEffect } from "react";
import { Bookmark } from "lucide-react";
import { SqlLoginPage } from "@/components/ui/sql-login-page";
import LumaBar from "@/components/ui/futuristic-nav";
import { GenericReportPage } from "@/components/ui/generic-report-page";
import { SettingsPage } from "@/components/ui/settings-page";
import { AIAssistantInterface } from "@/components/ui/ai-assistant-interface";
import { SchedulerPage } from "@/components/ui/scheduler-page";
import { invoke } from "@tauri-apps/api/core";
import { FIXED_AI_MODEL } from "@/lib/ai-config";
import "./App.css";

export interface ConnectionInfo {
  server: string;
  port: number;
  database: string;
  username: string;
  password: string;
  use_windows_auth: boolean;
  server_version: string | null;
}


type Page = "reports" | "search" | "alerts" | "ai" | "saved" | "settings";
const PAGES: Page[] = ["reports", "search", "alerts", "ai", "saved", "settings"];

export default function App() {
  const [connected,  setConnected]  = useState(false);
  const [connInfo,   setConnInfo]   = useState<ConnectionInfo | null>(null);
  const [pageIndex,  setPageIndex]  = useState(0);
  const [groqKey, setGroqKey] = useState("");
  const aiModel = FIXED_AI_MODEL;

  useEffect(() => {
    if (!connected) return;
    async function loadKey() {
      try {
        const remote = await invoke<{
          openrouter_api_key: string;
        }>("load_app_secrets_settings");
        setGroqKey(remote.openrouter_api_key ?? "");
      } catch (err) {
        console.error("Failed to load Supabase secrets:", err);
      }
    }
    loadKey();
  }, [connected]);

  if (!connected) {
    return (
      <SqlLoginPage
        onConnected={(info) => { setConnInfo(info); setConnected(true); }}
      />
    );
  }

  const currentPage = PAGES[pageIndex] ?? "reports";

  return (
    <div className="min-h-screen bg-background" dir="rtl">
      <main className="pb-28">
        {currentPage === "reports"  && <SchedulerPage />}
        {currentPage === "search"   && (
            <div className="flex flex-col h-screen overflow-y-auto pt-6">
              <GenericReportPage
                connInfo={connInfo!}
                reportName="Product Comprehensive Details"
                reportNameAr="البحث عن تفاصيل المنتجات"
              />
            </div>
        )}
        {currentPage === "alerts"   && <SchedulerPage />}
        {/* يبقى مُحمّلاً عند تغيير التبويب حتى لا تُفقد المحادثات الجارية */}
        <div className={currentPage === "ai" ? "contents" : "hidden"} aria-hidden={currentPage !== "ai"}>
          <AIAssistantInterface groqKey={groqKey} aiModel={aiModel} />
        </div>
        {currentPage === "saved"    && <PlaceholderPage title="المحفوظات"     Icon={Bookmark} />}
        {currentPage === "settings" && <SettingsPage />}
      </main>
      <LumaBar activeIndex={pageIndex} onNavigate={setPageIndex} />
    </div>
  );
}

// ── placeholder ───────────────────────────────────────────
function PlaceholderPage({ title, Icon }: { title: string; Icon: React.ElementType }) {
  return (
    <div className="flex flex-col items-center justify-center min-h-screen gap-4 text-muted-foreground">
      <div className="w-16 h-16 rounded-2xl bg-muted flex items-center justify-center">
        <Icon className="w-8 h-8 text-muted-foreground/50" />
      </div>
      <h2 className="text-xl font-bold text-foreground">{title}</h2>
      <p className="text-sm">قريباً</p>
    </div>
  );
}
