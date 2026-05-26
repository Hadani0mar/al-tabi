/**
 * صفحة الجدولة (قسم التقارير) — تعرض التقارير المجدوَلة مع عداد تنازلي
 * وقسم الإشعارات — تعرض التقارير التي صدرت مع إمكانية فتح الملف أو قراءة النص
 */

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Clock,
  Trash2,
  BellRing,
  FileText,
  FileSpreadsheet,
  RefreshCw,
  CheckCheck,
  Play,
  Pause,
  ChevronDown,
  ChevronUp,
  AlertCircle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

// بديل بسيط للـ Badge
function Badge({ children, variant = "secondary", className = "" }: {
  children: React.ReactNode;
  variant?: "secondary" | "destructive";
  className?: string;
}) {
  const base = "inline-flex items-center justify-center rounded-full px-1.5 py-0.5 text-[10px] font-medium";
  const variants = {
    secondary: "bg-muted text-muted-foreground",
    destructive: "bg-destructive text-destructive-foreground",
  };
  return <span className={cn(base, variants[variant], className)}>{children}</span>;
}

// ─── أنواع البيانات ────────────────────────────────────────────────────────────
interface ScheduledReport {
  id: string;
  name: string;
  description: string;
  report_title: string;
  report_type: "text" | "pdf" | "excel";
  interval_seconds: number;
  next_run_unix: number;
  last_run_unix: number | null;
  created_at_unix: number;
  is_active: boolean;
}

interface ReportNotification {
  id: string;
  schedule_id: string;
  schedule_name: string;
  title: string;
  generated_at_unix: number;
  report_type: "text" | "pdf" | "excel";
  text_content: string | null;
  file_path: string | null;
  is_read: boolean;
}

// ─── دالة تحويل الثواني إلى وصف عربي ─────────────────────────────────────────
function describeInterval(seconds: number): string {
  if (seconds >= 86400 * 7) return `كل ${Math.floor(seconds / 86400)} أيام`;
  if (seconds >= 86400) {
    const d = Math.floor(seconds / 86400);
    return d === 1 ? "يومياً" : `كل ${d} أيام`;
  }
  if (seconds >= 3600) {
    const h = Math.floor(seconds / 3600);
    return h === 1 ? "كل ساعة" : `كل ${h} ساعات`;
  }
  if (seconds >= 60) {
    const m = Math.floor(seconds / 60);
    return m === 1 ? "كل دقيقة" : `كل ${m} دقائق`;
  }
  return `كل ${seconds} ثانية`;
}

// ─── عداد تنازلي ──────────────────────────────────────────────────────────────
function formatCountdown(secsLeft: number): string {
  if (secsLeft <= 0) return "الآن";
  const d = Math.floor(secsLeft / 86400);
  const h = Math.floor((secsLeft % 86400) / 3600);
  const m = Math.floor((secsLeft % 3600) / 60);
  const s = secsLeft % 60;
  if (d > 0) return `${d}ي ${h}س`;
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

// ─── شريط التقدم ──────────────────────────────────────────────────────────────
function ProgressBar({
  nextRun,
  interval,
}: {
  nextRun: number;
  interval: number;
}) {
  const [pct, setPct] = useState(0);

  useEffect(() => {
    function calc() {
      const now = Math.floor(Date.now() / 1000);
      const elapsed = interval - Math.max(0, nextRun - now);
      setPct(Math.min(100, Math.max(0, (elapsed / interval) * 100)));
    }
    calc();
    const t = setInterval(calc, 1000);
    return () => clearInterval(t);
  }, [nextRun, interval]);

  return (
    <div className="w-full h-1.5 rounded-full bg-muted overflow-hidden">
      <div
        className="h-full rounded-full bg-primary transition-all duration-1000"
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}

// ─── بطاقة تقرير مجدوَل ───────────────────────────────────────────────────────
function ScheduleCard({
  sched,
  onDelete,
  onToggle,
}: {
  sched: ScheduledReport;
  onDelete: (id: string) => void;
  onToggle: (id: string, active: boolean) => void;
}) {
  const [secsLeft, setSecsLeft] = useState(0);

  useEffect(() => {
    function update() {
      const now = Math.floor(Date.now() / 1000);
      setSecsLeft(Math.max(0, sched.next_run_unix - now));
    }
    update();
    const t = setInterval(update, 1000);
    return () => clearInterval(t);
  }, [sched.next_run_unix]);

  const typeIcon =
    sched.report_type === "pdf" ? (
      <FileText className="w-4 h-4 text-red-400" />
    ) : sched.report_type === "excel" ? (
      <FileSpreadsheet className="w-4 h-4 text-green-500" />
    ) : (
      <FileText className="w-4 h-4 text-blue-400" />
    );

  const typeBadge =
    sched.report_type === "pdf"
      ? "PDF"
      : sched.report_type === "excel"
        ? "Excel"
        : "نص";

  return (
    <div
      className={cn(
        "rounded-xl border p-4 space-y-3 transition-opacity",
        sched.is_active ? "border-border bg-card" : "border-border/40 bg-muted/30 opacity-60"
      )}
    >
      {/* رأس البطاقة */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          {typeIcon}
          <span className="font-semibold text-sm truncate">{sched.name}</span>
          <Badge variant="secondary" className="text-[10px] shrink-0">
            {typeBadge}
          </Badge>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={() => onToggle(sched.id, !sched.is_active)}
            title={sched.is_active ? "إيقاف مؤقت" : "تفعيل"}
          >
            {sched.is_active ? (
              <Pause className="w-3.5 h-3.5" />
            ) : (
              <Play className="w-3.5 h-3.5" />
            )}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-destructive hover:text-destructive"
            onClick={() => onDelete(sched.id)}
            title="حذف"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </Button>
        </div>
      </div>

      {/* وصف + الفترة */}
      <div className="text-xs text-muted-foreground flex items-center gap-2">
        <Clock className="w-3.5 h-3.5 shrink-0" />
        <span>{describeInterval(sched.interval_seconds)}</span>
        {sched.description && (
          <>
            <span className="text-border">·</span>
            <span className="truncate">{sched.description}</span>
          </>
        )}
      </div>

      {/* شريط التقدم + عداد */}
      {sched.is_active && (
        <div className="space-y-1">
          <ProgressBar nextRun={sched.next_run_unix} interval={sched.interval_seconds} />
          <div className="flex justify-between text-[11px] text-muted-foreground">
            <span>التشغيل القادم خلال</span>
            <span className="font-mono font-semibold text-primary">
              {formatCountdown(secsLeft)}
            </span>
          </div>
        </div>
      )}

      {/* آخر تشغيل */}
      {sched.last_run_unix && (
        <div className="text-[11px] text-muted-foreground/70">
          آخر تشغيل:{" "}
          {new Date(sched.last_run_unix * 1000).toLocaleString("ar-LY", {
            dateStyle: "short",
            timeStyle: "short",
          })}
        </div>
      )}
    </div>
  );
}

// ─── بطاقة إشعار ──────────────────────────────────────────────────────────────
function NotificationCard({
  notif,
  onMarkRead,
  onOpenFile,
}: {
  notif: ReportNotification;
  onMarkRead: (id: string) => void;
  onOpenFile: (path: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const isError = notif.title.startsWith("⚠️");

  const typeIcon =
    notif.report_type === "pdf" ? (
      <FileText className="w-4 h-4 text-red-400" />
    ) : notif.report_type === "excel" ? (
      <FileSpreadsheet className="w-4 h-4 text-green-500" />
    ) : isError ? (
      <AlertCircle className="w-4 h-4 text-yellow-500" />
    ) : (
      <FileText className="w-4 h-4 text-blue-400" />
    );

  return (
    <div
      className={cn(
        "rounded-xl border p-3 space-y-2 transition-all",
        notif.is_read
          ? "border-border/50 bg-muted/20 opacity-75"
          : "border-primary/30 bg-primary/5"
      )}
    >
      {/* رأس */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          {typeIcon}
          <div className="min-w-0">
            <p className="text-sm font-medium truncate">{notif.title}</p>
            <p className="text-[11px] text-muted-foreground">
              {notif.schedule_name} ·{" "}
              {new Date(notif.generated_at_unix * 1000).toLocaleString("ar-LY", {
                dateStyle: "short",
                timeStyle: "short",
              })}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          {!notif.is_read && (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              onClick={() => onMarkRead(notif.id)}
              title="تعليم كمقروء"
            >
              <CheckCheck className="w-3.5 h-3.5" />
            </Button>
          )}
        </div>
      </div>

      {/* ملف أو نص */}
      {notif.file_path && (
        <Button
          variant="outline"
          size="sm"
          className="w-full gap-2 text-xs h-8"
          onClick={() => {
            onOpenFile(notif.file_path!);
            onMarkRead(notif.id);
          }}
        >
          {notif.report_type === "pdf" ? (
            <FileText className="w-3.5 h-3.5 text-red-400" />
          ) : (
            <FileSpreadsheet className="w-3.5 h-3.5 text-green-500" />
          )}
          فتح الملف
        </Button>
      )}

      {notif.text_content && (
        <div>
          <button
            className="w-full flex items-center justify-between text-xs text-muted-foreground hover:text-foreground transition-colors"
            onClick={() => {
              setExpanded((v) => !v);
              if (!notif.is_read) onMarkRead(notif.id);
            }}
          >
            <span>{expanded ? "إخفاء المحتوى" : "عرض المحتوى"}</span>
            {expanded ? (
              <ChevronUp className="w-3.5 h-3.5" />
            ) : (
              <ChevronDown className="w-3.5 h-3.5" />
            )}
          </button>
          {expanded && (
            <pre className="mt-2 text-[11px] bg-muted rounded-lg p-3 overflow-auto max-h-48 whitespace-pre-wrap font-mono leading-relaxed">
              {notif.text_content}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}

// ─── الصفحة الرئيسية ──────────────────────────────────────────────────────────
export function SchedulerPage() {
  const [schedules, setSchedules] = useState<ScheduledReport[]>([]);
  const [notifications, setNotifications] = useState<ReportNotification[]>([]);
  const [activeTab, setActiveTab] = useState<"schedules" | "notifications">("schedules");
  const [loading, setLoading] = useState(true);

  const loadData = useCallback(async () => {
    try {
      const [scheds, notifs] = await Promise.all([
        invoke<ScheduledReport[]>("get_scheduled_reports"),
        invoke<ReportNotification[]>("get_notifications"),
      ]);
      setSchedules(scheds);
      setNotifications(notifs);
    } catch (e) {
      console.error("scheduler load error", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();

    // استمع لأحداث التقارير الجديدة
    const unlisten = listen<ReportNotification>("report-notification", (event) => {
      setNotifications((prev) => [event.payload, ...prev.slice(0, 99)]);
      // انتقل تلقائياً لتبويب الإشعارات
      setActiveTab("notifications");
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, [loadData]);

  const handleDelete = async (id: string) => {
    try {
      await invoke("delete_scheduled_report", { id });
      setSchedules((prev) => prev.filter((s) => s.id !== id));
    } catch (e) {
      console.error(e);
    }
  };

  const handleToggle = async (id: string, active: boolean) => {
    try {
      await invoke("toggle_scheduled_report", { id, active });
      setSchedules((prev) =>
        prev.map((s) => (s.id === id ? { ...s, is_active: active } : s))
      );
    } catch (e) {
      console.error(e);
    }
  };

  const handleMarkRead = async (id: string) => {
    try {
      await invoke("mark_notification_read", { id });
      setNotifications((prev) =>
        prev.map((n) => (n.id === id ? { ...n, is_read: true } : n))
      );
    } catch (e) {
      console.error(e);
    }
  };

  const handleClearAll = async () => {
    try {
      await invoke("clear_all_notifications");
      setNotifications([]);
    } catch (e) {
      console.error(e);
    }
  };

  const handleOpenFile = async (path: string) => {
    try {
      await invoke("open_local_file", { path });
    } catch (e) {
      console.error(e);
    }
  };

  const unreadCount = notifications.filter((n) => !n.is_read).length;

  return (
    <div className="flex flex-col h-screen overflow-hidden">
      {/* تبويبات */}
      <div className="flex border-b border-border shrink-0 px-4 pt-4 gap-1">
        <button
          onClick={() => setActiveTab("schedules")}
          className={cn(
            "px-4 py-2 text-sm font-medium rounded-t-lg transition-colors",
            activeTab === "schedules"
              ? "bg-background border border-b-background border-border -mb-px text-foreground"
              : "text-muted-foreground hover:text-foreground"
          )}
        >
          التقارير المجدوَلة
          {schedules.length > 0 && (
            <Badge variant="secondary" className="mr-2 text-[10px]">
              {schedules.length}
            </Badge>
          )}
        </button>
        <button
          onClick={() => setActiveTab("notifications")}
          className={cn(
            "px-4 py-2 text-sm font-medium rounded-t-lg transition-colors flex items-center gap-1.5",
            activeTab === "notifications"
              ? "bg-background border border-b-background border-border -mb-px text-foreground"
              : "text-muted-foreground hover:text-foreground"
          )}
        >
          الإشعارات
          {unreadCount > 0 && (
            <Badge variant="destructive" className="text-[10px] h-4 min-w-[16px] px-1">
              {unreadCount}
            </Badge>
          )}
        </button>
        <div className="flex-1" />
        <button
          onClick={loadData}
          className="p-2 text-muted-foreground hover:text-foreground transition-colors"
          title="تحديث"
        >
          <RefreshCw className="w-4 h-4" />
        </button>
      </div>

      {/* المحتوى */}
      <div className="flex-1 overflow-y-auto p-4">
        {loading ? (
          <div className="flex items-center justify-center h-full text-muted-foreground text-sm gap-2">
            <RefreshCw className="w-4 h-4 animate-spin" />
            جاري التحميل...
          </div>
        ) : activeTab === "schedules" ? (
          // ─── التقارير المجدوَلة ───────────────────────────────────
          schedules.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full gap-4 text-muted-foreground">
              <Clock className="w-12 h-12 opacity-20" />
              <p className="text-sm opacity-60">لا توجد تقارير مجدوَلة</p>
              <p className="text-xs opacity-40 text-center max-w-60">
                اطلب من المساعد الذكي جدولة تقرير، مثلاً:
                <br />
                «جدوِل لي تقرير مبيعات يومي»
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {schedules.map((s) => (
                <ScheduleCard
                  key={s.id}
                  sched={s}
                  onDelete={handleDelete}
                  onToggle={handleToggle}
                />
              ))}
            </div>
          )
        ) : (
          // ─── الإشعارات ────────────────────────────────────────────
          <>
            {notifications.length > 0 && (
              <div className="flex justify-end mb-3">
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-xs gap-1.5 text-muted-foreground"
                  onClick={handleClearAll}
                >
                  <Trash2 className="w-3.5 h-3.5" />
                  مسح الكل
                </Button>
              </div>
            )}
            {notifications.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-full gap-4 text-muted-foreground">
                <BellRing className="w-12 h-12 opacity-20" />
                <p className="text-sm opacity-60">لا توجد إشعارات</p>
                <p className="text-xs opacity-40 text-center">
                  ستظهر هنا التقارير عند صدورها تلقائياً
                </p>
              </div>
            ) : (
              <div className="space-y-3">
                {notifications.map((n) => (
                  <NotificationCard
                    key={n.id}
                    notif={n}
                    onMarkRead={handleMarkRead}
                    onOpenFile={handleOpenFile}
                  />
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
