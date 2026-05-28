import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { load } from "@tauri-apps/plugin-store";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { motion, AnimatePresence } from "framer-motion";
import { Button } from "./button";
import { Input } from "./input";
import { Label } from "./label";
import { Checkbox } from "./checkbox";
import {
  Loader2, Check, Download, RefreshCw,
  LogOut, UserCircle2, Database, Server, ShieldCheck, AlertTriangle, X,
  ChevronLeft, Bot, ArrowUpCircle, Palette, Building2, MapPin, Briefcase, Phone,
} from "lucide-react";
import { cn } from "@/lib/utils";
import {
  applyTheme,
  loadActiveTheme,
  saveActiveTheme,
  THEME_OPTIONS,
  type ThemeId,
} from "@/lib/themes";
import type { ConnectionInfo } from "@/App";

interface TelegramSettingsLocal {
  bot_token: string;
  chat_id: string;
}

interface BusinessProfile {
  company_name: string | null;
  address: string | null;
  city: string | null;
  activity_code: string | null;
  activity_name: string | null;
  phone: string | null;
  mobile: string | null;
  fax: string | null;
  branch: string | null;
}

function displayValue(value: string | null | undefined) {
  return value?.trim() ? value : "—";
}

type SettingsView = "menu" | "bot" | "updates" | "account" | "themes";

interface SettingsPageProps {
  connInfo?: ConnectionInfo | null;
  onLogout?: () => void | Promise<void>;
}

const SECTIONS: {
  id: Exclude<SettingsView, "menu">;
  title: string;
  description: string;
  icon: React.ReactNode;
  iconBg: string;
}[] = [
  {
    id: "bot",
    title: "بوت Telegram",
    description: "توكن البوت، معرف الدردشة، وتفعيل الاستعلامات",
    icon: <img src="/telegram.svg" alt="" className="w-5 h-5" />,
    iconBg: "bg-sky-500/15 text-sky-600",
  },
  {
    id: "updates",
    title: "التحديثات",
    description: "التحقق من الإصدار وتثبيت التحديثات",
    icon: <ArrowUpCircle className="w-5 h-5" />,
    iconBg: "bg-emerald-500/15 text-emerald-600",
  },
  {
    id: "themes",
    title: "الثيمات",
    description: "مظهر التطبيق والألوان",
    icon: <Palette className="w-5 h-5" />,
    iconBg: "bg-amber-500/15 text-amber-700",
  },
  {
    id: "account",
    title: "الحساب",
    description: "جلسة الاتصال وتسجيل الخروج",
    icon: <UserCircle2 className="w-5 h-5" />,
    iconBg: "bg-indigo-500/15 text-indigo-600",
  },
];

function SectionHeader({
  title,
  onBack,
}: {
  title: string;
  onBack: () => void;
}) {
  return (
    <div className="flex items-center gap-2 pb-4 mb-2 shrink-0">
      <button
        type="button"
        onClick={onBack}
        className="w-9 h-9 rounded-lg flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-colors shrink-0"
        aria-label="رجوع"
      >
        <ChevronLeft className="w-5 h-5" />
      </button>
      <h2 className="text-xl sm:text-2xl font-semibold">{title}</h2>
    </div>
  );
}

export function SettingsPage({ connInfo, onLogout }: SettingsPageProps = {}) {
  const [view, setView] = useState<SettingsView>("menu");

  const [botToken, setBotToken] = useState("");
  const [chatId, setChatId] = useState("");
  const [enableQueries, setEnableQueries] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [saved, setSaved] = useState(false);
  const [logoutDialog, setLogoutDialog] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);
  const businessCacheRef = useRef<BusinessProfile | null>(null);

  const [appVersion, setAppVersion] = useState("");
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [updateAvailable, setUpdateAvailable] = useState<{
    version: string;
    notes: string;
    date?: string;
  } | null>(null);
  const [updateDownloading, setUpdateDownloading] = useState(false);
  const [updateProgress, setUpdateProgress] = useState(0);
  const [activeTheme, setActiveTheme] = useState<ThemeId>("default");
  const [businessProfile, setBusinessProfile] = useState<BusinessProfile | null>(null);
  const [businessLoading, setBusinessLoading] = useState(false);
  const [businessError, setBusinessError] = useState<string | null>(null);

  useEffect(() => {
    getVersion().then(setAppVersion).catch(console.error);
  }, []);

  useEffect(() => {
    async function loadSettings() {
      try {
        const store = await load("settings.json");
        const queriesEnabled = await store.get<boolean>("telegram_enable_queries");
        if (queriesEnabled !== null && queriesEnabled !== undefined) {
          setEnableQueries(queriesEnabled);
        }

        const local = await invoke<TelegramSettingsLocal>("load_telegram_settings_local");
        setBotToken(local.bot_token ?? "");
        setChatId(local.chat_id ?? "");

        const themeId = await loadActiveTheme();
        setActiveTheme(themeId);
        applyTheme(themeId);
      } catch (err) {
        console.error("Failed to load settings:", err);
      }
    }

    loadSettings();
  }, []);

  const loadBusinessProfile = useCallback(async (force = false) => {
    if (!force && businessCacheRef.current) {
      setBusinessProfile(businessCacheRef.current);
      setBusinessError(null);
      return;
    }

    setBusinessLoading(true);
    setBusinessError(null);

    try {
      const profile = await invoke<BusinessProfile>("get_business_profile");
      businessCacheRef.current = profile;
      setBusinessProfile(profile);
    } catch (err) {
      console.error("Failed to load business profile:", err);
      setBusinessError(String(err));
      if (force) {
        setBusinessProfile(null);
      }
    } finally {
      setBusinessLoading(false);
    }
  }, []);

  useEffect(() => {
    if (view !== "account") return;
    loadBusinessProfile();
  }, [view, loadBusinessProfile]);

  const handleCheckForUpdate = async () => {
    setUpdateChecking(true);
    setUpdateStatus(null);
    setUpdateAvailable(null);
    try {
      const update = await check();
      if (update) {
        setUpdateAvailable({
          version: update.version,
          notes: update.body ?? "",
          date: update.date ?? undefined,
        });
        setUpdateStatus(`يتوفر إصدار جديد: ${update.version}`);
      } else {
        setUpdateStatus("التطبيق محدّث إلى أحدث إصدار.");
      }
    } catch (err) {
      console.error("Check update failed:", err);
      setUpdateStatus(`تعذّر التحقق من التحديثات: ${err}`);
    } finally {
      setUpdateChecking(false);
    }
  };

  const handleInstallUpdate = async () => {
    setUpdateDownloading(true);
    setUpdateProgress(0);
    try {
      const update = await check();
      if (!update) {
        setUpdateStatus("لا يوجد تحديث متاح.");
        return;
      }
      let totalBytes = 0;
      let downloadedBytes = 0;
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            totalBytes = event.data.contentLength ?? 0;
            setUpdateStatus("بدء التنزيل...");
            break;
          case "Progress":
            downloadedBytes += event.data.chunkLength;
            if (totalBytes > 0) {
              setUpdateProgress(Math.round((downloadedBytes / totalBytes) * 100));
            }
            break;
          case "Finished":
            setUpdateStatus("اكتمل التنزيل — جارٍ إعادة التشغيل...");
            break;
        }
      });
      await relaunch();
    } catch (err) {
      console.error("Install update failed:", err);
      setUpdateStatus(`فشل التحديث: ${err}`);
    } finally {
      setUpdateDownloading(false);
    }
  };

  const handleSelectTheme = async (themeId: ThemeId) => {
    if (themeId === activeTheme) return;
    const previous = activeTheme;
    setActiveTheme(themeId);
    applyTheme(themeId);
    try {
      await saveActiveTheme(themeId);
    } catch (err) {
      console.error("Failed to save theme:", err);
      setActiveTheme(previous);
      applyTheme(previous);
      alert("تعذّر حفظ الثيم: " + err);
    }
  };

  const handleSaveBot = async () => {
    setSaving(true);
    try {
      await invoke("save_telegram_settings_local", {
        botToken,
        chatId,
        enableQueries,
      });
      await invoke("update_telegram_settings").catch(console.error);
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (err) {
      console.error("Failed to save bot settings:", err);
      alert("حدث خطأ أثناء حفظ إعدادات البوت: " + err);
    } finally {
      setSaving(false);
    }
  };

  const handleLogout = async () => {
    setLoggingOut(true);
    try {
      await onLogout?.();
    } catch (err) {
      console.error("Logout failed:", err);
      alert("تعذّر تسجيل الخروج: " + err);
    } finally {
      setLoggingOut(false);
      setLogoutDialog(false);
    }
  };

  const handleTest = async () => {
    if (!botToken || !chatId) {
      alert("يرجى إدخال توكن البوت ومعرف الدردشة أولاً");
      return;
    }
    setTesting(true);
    try {
      const msg = await invoke<string>("test_telegram_bot", { token: botToken, chatId });
      alert(msg);
    } catch (err) {
      console.error(err);
      alert("فشل الإرسال: " + err);
    } finally {
      setTesting(false);
    }
  };

  return (
    <div
      className="flex flex-col w-full min-h-[calc(100vh-7rem)] px-4 sm:px-6 pt-4 pb-6"
      dir="rtl"
    >
      <AnimatePresence mode="wait">
        {view === "menu" ? (
          <motion.div
            key="menu"
            initial={{ opacity: 0, x: 12 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -12 }}
            transition={{ duration: 0.2 }}
            className="flex flex-col flex-1 gap-6 min-h-0"
          >
            <div className="pt-2 shrink-0 px-1">
              <h1 className="text-2xl sm:text-3xl font-semibold">الإعدادات</h1>
            </div>

            <nav className="flex-1 w-full rounded-xl border border-border bg-card overflow-hidden divide-y divide-border">
              {SECTIONS.map((section) => (
                <button
                  key={section.id}
                  type="button"
                  onClick={() => setView(section.id)}
                  className={cn(
                    "w-full flex items-center gap-4 px-4 py-4 sm:px-5 sm:py-[18px]",
                    "text-right transition-colors hover:bg-muted/50 active:bg-muted/70"
                  )}
                >
                  <div
                    className={cn(
                      "w-10 h-10 rounded-lg flex items-center justify-center shrink-0",
                      section.iconBg
                    )}
                  >
                    {section.icon}
                  </div>
                  <div className="flex-1 min-w-0 text-right">
                    <div className="font-medium text-[15px]">{section.title}</div>
                    <div className="text-sm text-muted-foreground mt-0.5 truncate">
                      {section.description}
                    </div>
                  </div>
                  <ChevronLeft className="w-4 h-4 text-muted-foreground/70 rotate-180 shrink-0" />
                </button>
              ))}
            </nav>
          </motion.div>
        ) : (
          <motion.div
            key={view}
            initial={{ opacity: 0, x: -12 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 12 }}
            transition={{ duration: 0.2 }}
            className="flex flex-col flex-1 min-h-[calc(100vh-7rem)] w-full"
          >
            {view === "bot" && (
              <>
                <SectionHeader
                  title="بوت Telegram"
                  onBack={() => setView("menu")}
                />
                <div className="flex-1 space-y-5 overflow-y-auto min-h-0">
                  <div className="flex items-center gap-3 rounded-xl border border-sky-500/20 bg-sky-500/5 p-3">
                    <Bot className="w-5 h-5 text-sky-600 shrink-0" />
                    <p className="text-xs text-muted-foreground leading-relaxed">
                      يُحفظ توكن البوت ومعرف الدردشة على جهازك فقط. كل مستخدم يربط بوته الخاص.
                    </p>
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="botToken">توكن البوت (Bot Token)</Label>
                    <Input
                      id="botToken"
                      type="password"
                      placeholder="123456789:ABCdefGHIjklMNOpqrsTUVwxyz..."
                      value={botToken}
                      onChange={(e) => setBotToken(e.target.value)}
                    />
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="chatId">معرف الدردشة (Chat ID)</Label>
                    <Input
                      id="chatId"
                      placeholder="مثال: 123456789"
                      value={chatId}
                      onChange={(e) => setChatId(e.target.value)}
                    />
                    <p className="text-xs text-muted-foreground">معرف المستخدم أو المجموعة لاستقبال التقارير</p>
                  </div>

                  <div className="flex items-center space-x-2 space-x-reverse pt-1">
                    <Checkbox
                      id="enableQueries"
                      checked={enableQueries}
                      onCheckedChange={(c) => setEnableQueries(c as boolean)}
                    />
                    <Label htmlFor="enableQueries" className="font-medium cursor-pointer">
                      تفعيل الاستعلامات عبر البوت
                    </Label>
                  </div>
                  <p className="text-xs text-muted-foreground mr-6">
                    عند التفعيل، يمكن طلب التقارير من البوت واستلامها كملف Excel أو PDF.
                  </p>
                </div>

                <div className="pt-6 flex justify-end gap-3 border-t border-border mt-auto shrink-0">
                  <Button variant="outline" onClick={handleTest} disabled={testing || !botToken || !chatId}>
                    {testing ? <Loader2 className="w-4 h-4 animate-spin ml-2" /> : null}
                    اختبار الإرسال
                  </Button>
                  <Button onClick={handleSaveBot} disabled={saving}>
                    {saving ? <Loader2 className="w-4 h-4 animate-spin ml-2" /> : null}
                    {saved ? <Check className="w-4 h-4 ml-2 text-green-500" /> : null}
                    {saved ? "تم الحفظ" : "حفظ إعدادات البوت"}
                  </Button>
                </div>
              </>
            )}

            {view === "updates" && (
              <>
                <SectionHeader
                  title="التحديثات"
                  onBack={() => setView("menu")}
                />
                <div className="space-y-5">
                  <div className="grid gap-2">
                    <Label>الإصدار الحالي</Label>
                    <div
                      className="flex h-10 w-full items-center rounded-md border border-border bg-muted/40 px-3 py-2 text-sm text-muted-foreground"
                      dir="ltr"
                    >
                      v{appVersion || "..."}
                    </div>
                  </div>

                  <div className="flex flex-wrap gap-3">
                    <Button
                      variant="outline"
                      onClick={handleCheckForUpdate}
                      disabled={updateChecking || updateDownloading}
                    >
                      {updateChecking ? (
                        <Loader2 className="w-4 h-4 animate-spin ml-2" />
                      ) : (
                        <RefreshCw className="w-4 h-4 ml-2" />
                      )}
                      تحقق من التحديثات
                    </Button>

                    {updateAvailable && (
                      <Button onClick={handleInstallUpdate} disabled={updateDownloading}>
                        {updateDownloading ? (
                          <Loader2 className="w-4 h-4 animate-spin ml-2" />
                        ) : (
                          <Download className="w-4 h-4 ml-2" />
                        )}
                        {updateDownloading
                          ? `جارٍ التنزيل... ${updateProgress}%`
                          : `تثبيت v${updateAvailable.version}`}
                      </Button>
                    )}
                  </div>

                  {updateStatus && (
                    <div className="rounded-xl border border-border bg-muted/40 p-3 text-sm">
                      {updateStatus}
                    </div>
                  )}

                  {updateAvailable?.notes && (
                    <div className="grid gap-2">
                      <Label>ملاحظات الإصدار</Label>
                      <div className="rounded-xl border border-border bg-muted/40 p-3 text-sm whitespace-pre-wrap" dir="auto">
                        {updateAvailable.notes}
                      </div>
                    </div>
                  )}
                </div>
              </>
            )}

            {view === "themes" && (
              <>
                <SectionHeader
                  title="الثيمات"
                  onBack={() => setView("menu")}
                />
                <nav className="rounded-xl border border-border bg-card overflow-hidden divide-y divide-border">
                  {THEME_OPTIONS.map((theme) => (
                    <button
                      key={theme.id}
                      type="button"
                      onClick={() => handleSelectTheme(theme.id)}
                      className={cn(
                        "w-full flex items-center gap-4 px-4 py-4 sm:px-5 sm:py-[18px]",
                        "text-right transition-colors hover:bg-muted/50 active:bg-muted/70",
                        activeTheme === theme.id && "bg-primary/5"
                      )}
                    >
                      <div
                        className={cn(
                          "w-10 h-10 rounded-lg flex items-center justify-center shrink-0",
                          theme.iconBg
                        )}
                      />
                      <div className="flex-1 min-w-0 text-right">
                        <div className="font-medium text-[15px]">{theme.title}</div>
                        <div className="text-sm text-muted-foreground mt-0.5">
                          {theme.description}
                        </div>
                      </div>
                      {activeTheme === theme.id ? (
                        <Check className="w-5 h-5 text-primary shrink-0" />
                      ) : (
                        <ChevronLeft className="w-4 h-4 text-muted-foreground/50 rotate-180 shrink-0" />
                      )}
                    </button>
                  ))}
                </nav>
              </>
            )}

            {view === "account" && (
              <>
                <SectionHeader
                  title="الحساب"
                  onBack={() => setView("menu")}
                />
                <div className="space-y-5">
                  <div className="rounded-2xl border border-border bg-gradient-to-br from-card to-muted/20 p-5">
                    <div className="flex items-start gap-4">
                      <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-violet-500 to-indigo-600 flex items-center justify-center shadow-md shadow-indigo-500/25 flex-shrink-0">
                        <UserCircle2 className="w-8 h-8 text-white" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 mb-1">
                          <h3 className="text-base font-bold">الجلسة الحالية</h3>
                          <span className="flex items-center gap-1 text-[10px] font-bold px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-600 dark:text-emerald-300">
                            <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
                            متصل
                          </span>
                        </div>
                        <p className="text-xs text-muted-foreground leading-relaxed">
                          {connInfo?.use_windows_auth
                            ? "متصل عبر مصادقة Windows الآمنة"
                            : connInfo?.username
                              ? `متصل باسم: ${connInfo.username}`
                              : "متصل بقاعدة البيانات"}
                        </p>
                      </div>
                    </div>

                    {connInfo && (
                      <div className="grid grid-cols-2 gap-3 mt-5">
                        <div className="rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <Server className="w-3 h-3" />
                            السيرفر
                          </div>
                          <p className="text-sm font-mono font-semibold truncate" dir="ltr">
                            {connInfo.server}:{connInfo.port}
                          </p>
                        </div>
                        <div className="rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <Database className="w-3 h-3" />
                            قاعدة البيانات
                          </div>
                          <p className="text-sm font-mono font-semibold truncate" dir="ltr">
                            {connInfo.database}
                          </p>
                        </div>
                        {connInfo.server_version && (
                          <div className="col-span-2 rounded-xl border border-border bg-background/50 p-3">
                            <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                              <ShieldCheck className="w-3 h-3" />
                              إصدار SQL Server
                            </div>
                            <p className="text-xs font-mono text-muted-foreground/90 break-all" dir="ltr">
                              {connInfo.server_version}
                            </p>
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  <div className="rounded-2xl border border-border bg-gradient-to-br from-card to-muted/20 p-5">
                    <div className="flex items-start gap-3 mb-4">
                      <div className="w-10 h-10 rounded-xl bg-amber-500/15 flex items-center justify-center shrink-0">
                        <Building2 className="w-5 h-5 text-amber-700" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <h3 className="text-base font-bold">بيانات النشاط التجاري</h3>
                        <p className="text-xs text-muted-foreground mt-0.5">
                          معلومات المنشأة المسجّلة في النظام
                        </p>
                      </div>
                      <button
                        type="button"
                        onClick={() => loadBusinessProfile(true)}
                        disabled={businessLoading}
                        className="w-9 h-9 rounded-lg flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-colors shrink-0 disabled:opacity-50"
                        aria-label="تحديث بيانات المنشأة"
                      >
                        <RefreshCw className={cn("w-4 h-4", businessLoading && "animate-spin")} />
                      </button>
                    </div>

                    {businessLoading ? (
                      <div className="flex items-center justify-center gap-2 py-8 text-muted-foreground">
                        <Loader2 className="w-4 h-4 animate-spin" />
                        <span className="text-sm">جارٍ تحميل بيانات المنشأة...</span>
                      </div>
                    ) : businessError ? (
                      <div className="space-y-3">
                        <div className="rounded-xl border border-red-500/20 bg-red-500/5 p-3 text-sm text-red-600 dark:text-red-400">
                          {businessError}
                        </div>
                        <Button variant="outline" size="sm" onClick={() => loadBusinessProfile(true)}>
                          <RefreshCw className="w-4 h-4 ml-2" />
                          إعادة المحاولة
                        </Button>
                      </div>
                    ) : (
                      <>
                      {!businessProfile?.company_name &&
                        !businessProfile?.activity_name &&
                        !businessProfile?.activity_code && (
                        <p className="text-xs text-muted-foreground mb-3 px-1">
                          لا توجد بيانات مسجّلة في إعدادات النظام (SITTEINGS) — أدخلها من برنامج المحاسبة الأصلي.
                        </p>
                      )}
                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                        <div className="sm:col-span-2 rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <Building2 className="w-3 h-3" />
                            اسم المنشأة
                          </div>
                          <p className="text-sm font-semibold">{displayValue(businessProfile?.company_name)}</p>
                        </div>

                        <div className="rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <Briefcase className="w-3 h-3" />
                            كود النشاط
                          </div>
                          <p className="text-sm font-mono font-semibold" dir="ltr">
                            {displayValue(businessProfile?.activity_code)}
                          </p>
                        </div>

                        <div className="rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <Briefcase className="w-3 h-3" />
                            اسم النشاط التجاري
                          </div>
                          <p className="text-sm font-semibold">{displayValue(businessProfile?.activity_name)}</p>
                        </div>

                        <div className="sm:col-span-2 rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <MapPin className="w-3 h-3" />
                            العنوان
                          </div>
                          <p className="text-sm">{displayValue(businessProfile?.address)}</p>
                        </div>

                        <div className="rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <MapPin className="w-3 h-3" />
                            المدينة
                          </div>
                          <p className="text-sm">{displayValue(businessProfile?.city)}</p>
                        </div>

                        <div className="rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <Building2 className="w-3 h-3" />
                            الفرع
                          </div>
                          <p className="text-sm font-mono font-semibold" dir="ltr">
                            {displayValue(businessProfile?.branch)}
                          </p>
                        </div>

                        <div className="rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <Phone className="w-3 h-3" />
                            الهاتف
                          </div>
                          <p className="text-sm font-mono" dir="ltr">{displayValue(businessProfile?.phone)}</p>
                        </div>

                        <div className="rounded-xl border border-border bg-background/50 p-3">
                          <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                            <Phone className="w-3 h-3" />
                            الجوال
                          </div>
                          <p className="text-sm font-mono" dir="ltr">{displayValue(businessProfile?.mobile)}</p>
                        </div>

                        {businessProfile?.fax && (
                          <div className="rounded-xl border border-border bg-background/50 p-3">
                            <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground mb-1">
                              <Phone className="w-3 h-3" />
                              الفاكس
                            </div>
                            <p className="text-sm font-mono" dir="ltr">{businessProfile.fax}</p>
                          </div>
                        )}
                      </div>
                      </>
                    )}
                  </div>

                  <div className="rounded-2xl border border-red-500/20 bg-red-500/[0.03] p-5">
                    <div className="flex items-start gap-3">
                      <div className="w-10 h-10 rounded-xl bg-red-500/10 flex items-center justify-center flex-shrink-0">
                        <LogOut className="w-5 h-5 text-red-500" />
                      </div>
                      <div className="flex-1">
                        <h4 className="text-sm font-bold text-foreground mb-1">تسجيل الخروج</h4>
                        <p className="text-xs text-muted-foreground leading-relaxed mb-4">
                          سيتم إنهاء الجلسة والعودة لشاشة تسجيل الدخول. بياناتك المحفوظة لن تُحذف.
                        </p>
                        <Button
                          variant="outline"
                          onClick={() => setLogoutDialog(true)}
                          className="gap-2 border-red-500/30 text-red-600 hover:bg-red-500/10 hover:text-red-700 hover:border-red-500/50 dark:text-red-400 dark:hover:text-red-300"
                        >
                          <LogOut className="w-4 h-4" />
                          تسجيل الخروج
                        </Button>
                      </div>
                    </div>
                  </div>
                </div>
              </>
            )}
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {logoutDialog && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
            onClick={() => !loggingOut && setLogoutDialog(false)}
          >
            <motion.div
              initial={{ opacity: 0, scale: 0.9, y: 16 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95 }}
              transition={{ type: "spring", stiffness: 280, damping: 24 }}
              className="relative w-full max-w-sm rounded-2xl bg-card border border-border shadow-2xl p-6"
              dir="rtl"
              onClick={(e) => e.stopPropagation()}
            >
              {!loggingOut && (
                <button
                  onClick={() => setLogoutDialog(false)}
                  className="absolute left-4 top-4 w-8 h-8 rounded-lg flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                >
                  <X className="w-4 h-4" />
                </button>
              )}

              <div className="flex flex-col items-center text-center">
                <motion.div
                  initial={{ scale: 0, rotate: -30 }}
                  animate={{ scale: 1, rotate: 0 }}
                  transition={{ type: "spring", stiffness: 320, damping: 18 }}
                  className="w-14 h-14 rounded-2xl bg-red-500/15 flex items-center justify-center mb-4"
                >
                  <AlertTriangle className="w-7 h-7 text-red-500" />
                </motion.div>

                <h3 className="text-lg font-bold mb-1.5">تأكيد تسجيل الخروج</h3>
                <p className="text-sm text-muted-foreground leading-relaxed mb-6">
                  سيتم إنهاء الجلسة الحالية والعودة لشاشة تسجيل الدخول.
                  <br />
                  <span className="text-xs">بيانات الاتصال المحفوظة ستبقى — يمكنك الدخول مرة أخرى دون إعادة الإدخال.</span>
                </p>

                <div className="flex gap-2.5 w-full">
                  <Button
                    variant="outline"
                    className="flex-1 h-10"
                    onClick={() => setLogoutDialog(false)}
                    disabled={loggingOut}
                  >
                    إلغاء
                  </Button>
                  <Button
                    className="flex-1 h-10 gap-2 bg-red-500 hover:bg-red-600 text-white"
                    onClick={handleLogout}
                    disabled={loggingOut}
                  >
                    {loggingOut ? (
                      <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                      <LogOut className="w-4 h-4" />
                    )}
                    {loggingOut ? "جارٍ الخروج..." : "تسجيل الخروج"}
                  </Button>
                </div>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
