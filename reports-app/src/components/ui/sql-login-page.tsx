import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { load } from "@tauri-apps/plugin-store";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Eye, EyeOff, Database,
  Server, Hash, User, Lock, ShieldCheck,
  Monitor, Save, Plug, Loader2,
  CheckCircle2, XCircle, AlertCircle,
  KeyRound, BookLock, Wifi,
  ChevronRight, ShieldAlert,
  Bot, Sparkles, Wand2, X, Zap, Check,
  MessageCircle,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { cn } from "@/lib/utils";

// ─── Types ─────────────────────────────────────────────────────
interface SqlConnection {
  server: string; port: number; database: string;
  username: string; password: string; use_windows_auth: boolean;
  disable_encryption: boolean;
}
interface ConnectionResult {
  success: boolean; message: string; server_version: string | null;
}
const STORE_FILE = "connections.dat";

function FieldIcon({ icon: Icon, className }: { icon: React.ElementType; className?: string }) {
  return <Icon className={cn("absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground/60 pointer-events-none", className)} />;
}

interface SqlLoginPageProps {
  autoConnecting?: boolean;
  onConnected?: (info: {
    server: string; port: number; database: string;
    username: string; password: string;
    use_windows_auth: boolean; disable_encryption: boolean;
    server_version: string | null;
  }) => void;
}

const sleep = (ms: number) => new Promise<void>(res => setTimeout(res, ms));

type AutoStatus = "idle" | "running" | "success" | "error";

interface AiStep {
  key: "server" | "port" | "database" | "winauth" | "connect";
  label: string;
  detail: string;
  icon: React.ElementType;
}

const AI_STEPS: AiStep[] = [
  { key: "server",   label: "تحديد عنوان السيرفر",         detail: "localhost",       icon: Server },
  { key: "port",     label: "ضبط المنفذ الافتراضي",         detail: "1433",            icon: Hash },
  { key: "database", label: "اختيار قاعدة البيانات",         detail: "Marketing2026",   icon: Database },
  { key: "winauth",  label: "تفعيل مصادقة Windows الآمنة",  detail: "Windows Auth",    icon: ShieldCheck },
  { key: "connect",  label: "اختبار الاتصال وتفعيل الجلسة", detail: "Test & Connect",  icon: Plug },
];

export function SqlLoginPage({ autoConnecting, onConnected }: SqlLoginPageProps) {
  const [form, setForm] = useState<SqlConnection>({ server: "", port: 1433, database: "", username: "", password: "", use_windows_auth: false, disable_encryption: false });
  const [showPass, setShowPass] = useState(false);
  const [status, setStatus] = useState<"idle" | "loading" | "success" | "error">("idle");
  const [msg, setMsg] = useState("");
  const [version, setVersion] = useState<string | null>(null);
  const [savedConn, setSavedConn] = useState(false);
  const [rememberMe, setRememberMe] = useState(true);
  const [autoLogin, setAutoLogin] = useState(false);
  const [autoOpen, setAutoOpen] = useState(false);
  const [autoStatus, setAutoStatus] = useState<AutoStatus>("idle");
  const [autoStep, setAutoStep] = useState<number>(-1);
  const [autoError, setAutoError] = useState<string>("");

  useEffect(() => { loadHistory(); }, []);

  async function loadHistory() {
    try {
      const store = await load(STORE_FILE, { autoSave: false, defaults: {} });
      const last = await store.get<string>("last_connection");
      const names = await store.get<string[]>("connection_names") ?? [];
      const auto = await store.get<boolean>("auto_login");
      if (auto !== null && auto !== undefined) setAutoLogin(!!auto);
      if (last && names.includes(last)) await loadConnection(last);
    } catch (_) {}
  }
  async function loadConnection(name: string) {
    try {
      const store = await load(STORE_FILE, { autoSave: false, defaults: {} });
      const enc = await store.get<Record<string, string>>(`conn_${name}`);
      if (!enc) return;
      const password = enc.password ? await invoke<string>("decrypt_value", { encrypted: enc.password }) : "";
      setForm({ server: enc.server ?? "", port: parseInt(enc.port ?? "1433"), database: enc.database ?? "", username: enc.username ?? "", password, use_windows_auth: enc.use_windows_auth === "true", disable_encryption: enc.disable_encryption === "true" });
      setSavedConn(true);
    } catch (_) {}
  }
  async function saveConnection(name: string) {
    try {
      const store = await load(STORE_FILE, { autoSave: false, defaults: {} });
      const encPassword = form.password ? await invoke<string>("encrypt_value", { value: form.password }) : "";
      await store.set(`conn_${name}`, { server: form.server, port: String(form.port), database: form.database, username: form.username, password: encPassword, use_windows_auth: String(form.use_windows_auth), disable_encryption: String(form.disable_encryption) });
      const names = await store.get<string[]>("connection_names") ?? [];
      if (!names.includes(name)) await store.set("connection_names", [...names, name]);
      await store.set("last_connection", name);
      await store.set("auto_login", autoLogin);
      await store.save();
      setSavedConn(true);
    } catch (e) { console.error(e); }
  }
  async function persistAutoLogin(value: boolean) {
    setAutoLogin(value);
    try {
      const store = await load(STORE_FILE, { autoSave: false, defaults: {} });
      await store.set("auto_login", value);
      await store.save();
    } catch (e) { console.error(e); }
  }

  const set = (k: keyof SqlConnection, v: string | number | boolean) => setForm(p => ({ ...p, [k]: v }));

  async function handleConnect(e: React.FormEvent) {
    e.preventDefault();
    setStatus("loading"); setMsg(""); setVersion(null);
    try {
      const result = await invoke<ConnectionResult>("test_sql_connection", { conn: form });
      if (result.success) {
        setStatus("success"); setMsg(result.message); setVersion(result.server_version);
        if (rememberMe) await saveConnection(form.database || form.server);
        
        // Pass connection to backend AppState for the Telegram Bot
        await invoke("set_active_connection", { conn: form }).catch(console.error);

        // الانتقال إلى الصفحة الرئيسية بعد لحظة
        setTimeout(() => {
          onConnected?.({
            server: form.server, port: form.port, database: form.database,
            username: form.username, password: form.password,
            use_windows_auth: form.use_windows_auth,
            disable_encryption: form.disable_encryption,
            server_version: result.server_version,
          });
        }, 1200);
      } else { setStatus("error"); setMsg(result.message); }
    } catch (err) { setStatus("error"); setMsg(`خطأ: ${err}`); }
  }

  async function typewriteField(field: "server" | "database", value: string) {
    for (let i = 1; i <= value.length; i++) {
      setForm(p => ({ ...p, [field]: value.substring(0, i) }));
      await sleep(55 + Math.random() * 35);
    }
  }

  function openAutoConnect() {
    setAutoStatus("idle");
    setAutoStep(-1);
    setAutoError("");
    setAutoOpen(true);
  }

  async function runAutoConnect() {
    setAutoStatus("running");
    setAutoError("");
    setStatus("idle");
    setMsg("");
    setForm({ server: "", port: 1433, database: "", username: "", password: "", use_windows_auth: false, disable_encryption: false });
    await sleep(450);

    try {
      // الخطوة 0: السيرفر
      setAutoStep(0);
      await sleep(350);
      await typewriteField("server", "localhost");
      await sleep(400);

      // الخطوة 1: المنفذ
      setAutoStep(1);
      setForm(p => ({ ...p, port: 1433 }));
      await sleep(900);

      // الخطوة 2: قاعدة البيانات
      setAutoStep(2);
      await sleep(350);
      await typewriteField("database", "Marketing2026");
      await sleep(400);

      // الخطوة 3: Windows Auth
      setAutoStep(3);
      await sleep(450);
      setForm(p => ({ ...p, use_windows_auth: true }));
      await sleep(700);

      // الخطوة 4: الاتصال الفعلي
      setAutoStep(4);
      await sleep(500);

      const conn: SqlConnection = {
        server: "localhost",
        port: 1433,
        database: "Marketing2026",
        username: "",
        password: "",
        use_windows_auth: true,
        disable_encryption: false,
      };

      const result = await invoke<ConnectionResult>("test_sql_connection", { conn });

      if (!result.success) {
        setAutoStatus("error");
        setAutoError(result.message);
        setStatus("error");
        setMsg(result.message);
        return;
      }

      setStatus("success");
      setMsg(result.message);
      setVersion(result.server_version);
      if (rememberMe) await saveConnection(conn.database);
      await invoke("set_active_connection", { conn }).catch(console.error);

      setAutoStatus("success");
      await sleep(1400);
      onConnected?.({ ...conn, server_version: result.server_version });
    } catch (err) {
      setAutoStatus("error");
      setAutoError(`${err}`);
      setStatus("error");
      setMsg(`خطأ: ${err}`);
    }
  }

  const brandFeatures = [
    { icon: Check, text: "قراءة فقط" },
    { icon: Database, text: "SQL Server" },
    { icon: MessageCircle, text: "Telegram" },
  ];

  return (
    <div className="min-h-screen grid lg:grid-cols-2" style={{ background: "var(--bg-canvas)" }}>

      {/* ══════════════ اليسار — Mihbar brand panel ══════════════ */}
      <div
        className="relative hidden lg:flex flex-col justify-between p-14 overflow-hidden"
        style={{
          background: "linear-gradient(160deg, #0F6E70 0%, #0A5759 100%)",
          color: "var(--fg-on-brand)",
        }}
      >
        <div
          className="pointer-events-none absolute -bottom-20 -left-20 h-80 w-80 rounded-full"
          style={{ background: "radial-gradient(circle, rgba(184,106,44,0.30), transparent 65%)" }}
        />

        <div className="relative z-10 flex items-center gap-3">
          <img src="/assets/logo-mark.svg" alt="" width={44} height={44} className="rounded-[10px]" />
          <div>
            <div className="text-[22px] font-bold leading-tight" style={{ fontFamily: "var(--font-display)" }}>
              نظام المتمكن
            </div>
            <div
              className="mt-0.5 text-[11px] font-medium opacity-70"
              style={{ fontFamily: "var(--font-mono)", letterSpacing: "0.04em" }}
            >
              NIZAM AL-MUTAMAKKUN
            </div>
          </div>
        </div>

        <div className="relative z-10">
          <h2
            className="mb-4 text-[32px] font-bold leading-snug"
            style={{ fontFamily: "var(--font-display)", letterSpacing: "-0.01em" }}
          >
            ذكاء التقارير
            <br />
            فوق Marketing2026
          </h2>
          <p className="max-w-[360px] text-[14.5px] leading-relaxed opacity-82">
            اسأل بالعربية. احصل على الجواب فوراً — جدول، PDF، أو Excel.
            بيانات على جهازك، اتصال قراءة فقط.
          </p>
          <div className="mt-8 flex flex-wrap gap-[18px]">
            {brandFeatures.map(({ icon: Icon, text }) => (
              <div key={text} className="flex items-center gap-1.5 text-xs opacity-85 whitespace-nowrap">
                <Icon size={16} />
                {text}
              </div>
            ))}
          </div>
        </div>

        <div className="relative z-10 text-[11px] opacity-55" style={{ fontFamily: "var(--font-mono)" }}>
          v0.1.2 · build 2026.05
        </div>
      </div>

      {/* ══════════════ اليمين — النموذج ══════════════ */}
      <div className="flex items-center justify-center p-8 lg:p-16" dir="rtl" style={{ background: "var(--bg-canvas)" }}>
        <div className="w-full max-w-[420px]">

          {/* موبايل شعار */}
          <div className="lg:hidden flex items-center justify-center gap-2 text-lg font-bold mb-10">
            <img src="/assets/logo-mark.svg" alt="" width={32} height={32} className="rounded-lg" />
            <span style={{ fontFamily: "var(--font-display)" }}>نظام المتمكن</span>
          </div>

          {/* ── العنوان ── */}
          <div className="mb-7">
            <div
              className="mb-2 text-[11px] font-semibold uppercase tracking-widest"
              style={{ color: "var(--brand-primary)", fontFamily: "var(--font-mono)" }}
            >
              الخطوة الأولى
            </div>
            <h1 className="text-2xl font-bold tracking-tight mb-2" style={{ color: "var(--fg-1)" }}>
              اتصل بقاعدة البيانات
            </h1>
            <p className="text-[13.5px] leading-relaxed" style={{ color: "var(--fg-2)" }}>
              أدخل بيانات SQL Server الخاصة بـ Marketing2026.
              يتم حفظ بيانات الاتصال بأمان على جهازك فقط.
            </p>
          </div>

          {/* ── النموذج ── */}
          <form onSubmit={handleConnect} className="space-y-4">

            {/* السيرفر + المنفذ */}
            <div className="flex gap-3">
              <div className="flex-1 space-y-1.5">
                <Label htmlFor="server" className="flex items-center gap-1.5">
                  <Server className="w-3.5 h-3.5 text-muted-foreground" />
                  السيرفر
                </Label>
                <div className="relative">
                  <Input id="server" placeholder="localhost" value={form.server}
                    onChange={e => set("server", e.target.value)}
                    className="h-11 pr-9" dir="ltr" required />
                  <FieldIcon icon={Wifi} />
                </div>
              </div>
              <div className="w-[90px] space-y-1.5">
                <Label htmlFor="port" className="flex items-center gap-1.5">
                  <Hash className="w-3.5 h-3.5 text-muted-foreground" />
                  منفذ
                </Label>
                <Input id="port" type="number" value={form.port}
                  onChange={e => set("port", parseInt(e.target.value))}
                  className="h-11 text-center" dir="ltr" />
              </div>
            </div>

            {/* قاعدة البيانات */}
            <div className="space-y-1.5">
              <Label htmlFor="database" className="flex items-center gap-1.5">
                <Database className="w-3.5 h-3.5 text-muted-foreground" />
                قاعدة البيانات
              </Label>
              <div className="relative">
                <Input id="database" placeholder="Marketing2026" value={form.database}
                  onChange={e => set("database", e.target.value)}
                  className="h-11 pr-9" dir="ltr" required />
                <FieldIcon icon={Database} />
              </div>
            </div>

            {/* مصادقة Windows */}
            <div className="flex items-center gap-2.5 p-3 rounded-lg border border-dashed border-border bg-muted/30">
              <Checkbox id="winauth" checked={form.use_windows_auth}
                onCheckedChange={v => set("use_windows_auth", !!v)} />
              <Label htmlFor="winauth" className="flex items-center gap-1.5 font-normal cursor-pointer">
                <Monitor className="w-3.5 h-3.5 text-muted-foreground" />
                استخدام مصادقة Windows
              </Label>
              <ShieldAlert className={cn("w-3.5 h-3.5 mr-auto transition-colors", form.use_windows_auth ? "text-primary" : "text-muted-foreground/30")} />
            </div>

            {/* تعطيل تشفير TLS — لـ SQL Server القديم */}
            <div className="flex items-center gap-2.5 p-3 rounded-lg border border-dashed border-border bg-muted/30">
              <Checkbox id="noencrypt" checked={form.disable_encryption}
                onCheckedChange={v => set("disable_encryption", !!v)} />
              <Label htmlFor="noencrypt" className="flex items-center gap-1.5 font-normal cursor-pointer">
                <ShieldCheck className="w-3.5 h-3.5 text-muted-foreground" />
                تعطيل تشفير الاتصال (TLS)
              </Label>
              <span className="text-[10px] text-muted-foreground mr-auto">للسيرفرات القديمة</span>
            </div>

            {/* بيانات تسجيل الدخول */}
            {!form.use_windows_auth && (
              <div className="space-y-3 p-3.5 rounded-lg bg-muted/20 border border-border/60">
                {/* اسم المستخدم */}
                <div className="space-y-1.5">
                  <Label htmlFor="username" className="flex items-center gap-1.5">
                    <User className="w-3.5 h-3.5 text-muted-foreground" />
                    اسم المستخدم
                  </Label>
                  <div className="relative">
                    <Input id="username" placeholder="sa" value={form.username}
                      onChange={e => set("username", e.target.value)}
                      className="h-11 pr-9" dir="ltr" />
                    <FieldIcon icon={User} />
                  </div>
                </div>
                {/* كلمة المرور */}
                <div className="space-y-1.5">
                  <Label htmlFor="password" className="flex items-center gap-1.5">
                    <Lock className="w-3.5 h-3.5 text-muted-foreground" />
                    كلمة المرور
                  </Label>
                  <div className="relative">
                    <Input id="password" type={showPass ? "text" : "password"}
                      placeholder="••••••••" value={form.password}
                      onChange={e => set("password", e.target.value)}
                      className="h-11 pl-10 pr-9" dir="ltr" />
                    <FieldIcon icon={KeyRound} />
                    <button type="button" onClick={() => setShowPass(!showPass)}
                      className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors">
                      {showPass ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                    </button>
                  </div>
                </div>
              </div>
            )}

            {/* حفظ + مؤشر الحفظ */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Checkbox id="remember" checked={rememberMe}
                  onCheckedChange={v => setRememberMe(!!v)} />
                <Label htmlFor="remember" className="flex items-center gap-1.5 text-sm font-normal cursor-pointer">
                  <Save className="w-3.5 h-3.5 text-muted-foreground" />
                  حفظ بيانات الاتصال
                </Label>
              </div>
              {savedConn && (
                <span className="flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400">
                  <ShieldCheck className="w-3.5 h-3.5" />
                  محفوظ مشفّراً
                </span>
              )}
            </div>

            {/* تذكّر حالة تسجيل الدخول — Auto Login */}
            <motion.label
              htmlFor="autologin"
              whileHover={{ scale: 1.005 }}
              whileTap={{ scale: 0.995 }}
              className={cn(
                "relative flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all overflow-hidden",
                autoLogin
                  ? "border-primary/50 bg-primary/5"
                  : "border-border bg-muted/20 hover:bg-muted/30",
              )}
            >
              {autoLogin && (
                <motion.div
                  className="absolute inset-0 bg-gradient-to-l from-transparent via-primary/8 to-transparent"
                  initial={{ x: "-100%" }}
                  animate={{ x: "100%" }}
                  transition={{ repeat: Infinity, duration: 3, ease: "linear" }}
                />
              )}
              <Checkbox id="autologin" checked={autoLogin}
                onCheckedChange={v => persistAutoLogin(!!v)}
                className="relative z-10 mt-0.5"
                disabled={!rememberMe} />
              <div className="relative z-10 flex-1">
                <div className="flex items-center gap-1.5">
                  <Zap className={cn(
                    "w-3.5 h-3.5 transition-colors",
                    autoLogin ? "text-primary" : "text-muted-foreground",
                  )} />
                  <span className="text-sm font-semibold">تذكّر حالة تسجيل الدخول</span>
                  {autoLogin && (
                    <motion.span
                      initial={{ opacity: 0, scale: 0.8 }}
                      animate={{ opacity: 1, scale: 1 }}
                      className="text-[10px] font-bold px-1.5 py-0.5 rounded-md bg-primary/15 text-primary"
                    >
                      مُفعّل
                    </motion.span>
                  )}
                </div>
                <p className="text-[11px] text-muted-foreground mt-0.5 leading-relaxed">
                  سيتم الاتصال تلقائياً عند فتح التطبيق في المرات القادمة — دون الحاجة لإعادة الإدخال.
                </p>
              </div>
            </motion.label>

            {/* رسالة الخطأ */}
            {status === "error" && (
              <div className="flex items-start gap-2.5 p-3 text-sm text-red-600 bg-red-50 border border-red-200 rounded-lg dark:bg-red-950/20 dark:border-red-900/30 dark:text-red-400">
                <XCircle className="w-4 h-4 flex-shrink-0 mt-0.5" />
                <span>{msg}</span>
              </div>
            )}

            {/* رسالة النجاح */}
            {status === "success" && (
              <div className="flex items-start gap-2.5 p-3 text-sm bg-emerald-50 border border-emerald-200 rounded-lg dark:bg-emerald-950/20 dark:border-emerald-900/30">
                <CheckCircle2 className="w-4 h-4 flex-shrink-0 mt-0.5 text-emerald-600 dark:text-emerald-400" />
                <div>
                  <p className="font-semibold text-emerald-700 dark:text-emerald-300">{msg}</p>
                  {version && <p className="text-xs mt-0.5 text-emerald-600/70 dark:text-emerald-400/70" dir="ltr">{version}</p>}
                </div>
              </div>
            )}

            {/* اتصال تلقائي في الخلفية */}
            {autoConnecting && status === "idle" && (
              <div className="flex items-center gap-2.5 p-3 text-sm text-primary bg-primary/5 border border-primary/20 rounded-lg">
                <Loader2 className="w-4 h-4 animate-spin flex-shrink-0" />
                <span>جارٍ الاتصال التلقائي بقاعدة البيانات...</span>
              </div>
            )}

            {/* رسالة تحميل */}
            {status === "loading" && (
              <div className="flex items-center gap-2.5 p-3 text-sm text-primary bg-primary/5 border border-primary/20 rounded-lg">
                <Loader2 className="w-4 h-4 animate-spin flex-shrink-0" />
                <span>جارٍ الاتصال بقاعدة البيانات...</span>
              </div>
            )}

            {/* ── زر الاتصال ── */}
            <Button type="submit"
              className="w-full h-12 text-base font-bold gap-2.5 shadow-md" size="lg"
              disabled={!form.server || !form.database || status === "loading" || autoConnecting}>
              {status === "loading"
                ? <><Loader2 className="w-5 h-5 animate-spin" /> جارٍ الاتصال…</>
                : <><Plug className="w-5 h-5" /> اتصل بقاعدة البيانات</>}
            </Button>

            {/* فاصل */}
            <div className="relative flex items-center my-2">
              <div className="flex-1 h-px bg-border" />
              <span className="px-3 text-[11px] text-muted-foreground font-medium">أو</span>
              <div className="flex-1 h-px bg-border" />
            </div>

            {/* ── زر الاتصال التلقائي بالمساعد الذكي ── */}
            <motion.button
              type="button"
              onClick={openAutoConnect}
              whileHover={{ scale: 1.015 }}
              whileTap={{ scale: 0.985 }}
              className="relative w-full h-12 rounded-lg overflow-hidden group border transition-colors"
              style={{
                borderColor: "color-mix(in srgb, var(--brand-accent) 35%, transparent)",
                background: "color-mix(in srgb, var(--brand-accent-soft) 40%, transparent)",
              }}
            >
              <div className="relative z-10 flex items-center justify-center gap-2.5 text-sm font-bold" style={{ color: "var(--brand-accent-ink)" }}>
                <div
                  className="w-8 h-8 rounded-lg flex items-center justify-center shadow-md"
                  style={{ background: "var(--brand-primary)" }}
                >
                  <Bot className="w-4 h-4 text-white" />
                </div>
                <span>اتصل تلقائياً بمساعدة الوكيل الذكي</span>
                <Sparkles className="w-3.5 h-3.5" style={{ color: "var(--brand-accent)" }} />
              </div>
            </motion.button>
          </form>

          {/* ── Footer ── */}
          <div className="flex items-center justify-center gap-1.5 text-xs text-muted-foreground mt-6">
            <BookLock className="w-3 h-3" />
            <span>البيانات مشفّرة محلياً</span>
            <span className="text-muted-foreground/30">·</span>
            <span className="font-mono">AES-256-GCM</span>
            <ChevronRight className="w-3 h-3 text-muted-foreground/30" />
            <AlertCircle className="w-3 h-3" />
            <span>لا ترسل إلى أي خادم خارجي</span>
          </div>
        </div>
      </div>

      {/* ══════════════ Modal — الاتصال التلقائي بالمساعد الذكي ══════════════ */}
      <AnimatePresence>
        {autoOpen && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/65 backdrop-blur-sm p-4"
            onClick={() => autoStatus !== "running" && setAutoOpen(false)}
          >
            <motion.div
              initial={{ opacity: 0, scale: 0.9, y: 24 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95, y: 12 }}
              transition={{ type: "spring", stiffness: 280, damping: 26 }}
              className="relative w-full max-w-md rounded-3xl bg-card border border-border shadow-2xl p-7 overflow-hidden"
              dir="rtl"
              onClick={e => e.stopPropagation()}
            >
              {/* خلفية مزخرفة */}
              <div className="absolute -top-28 -left-20 w-72 h-72 rounded-full blur-3xl pointer-events-none" style={{ background: "color-mix(in srgb, var(--brand-primary) 20%, transparent)" }} />
              <div className="absolute -bottom-28 -right-20 w-72 h-72 rounded-full blur-3xl pointer-events-none" style={{ background: "color-mix(in srgb, var(--brand-accent) 15%, transparent)" }} />

              {/* زر الإغلاق */}
              {autoStatus !== "running" && (
                <button
                  onClick={() => setAutoOpen(false)}
                  className="absolute left-4 top-4 w-8 h-8 rounded-lg flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-colors z-20"
                >
                  <X className="w-4 h-4" />
                </button>
              )}

              {/* رأس Modal — أيقونة الوكيل */}
              <div className="relative z-10 flex flex-col items-center text-center mb-6">
                <div className="relative mb-3">
                  <motion.div
                    animate={autoStatus === "running" ? { scale: [1, 1.06, 1] } : {}}
                    transition={{ repeat: Infinity, duration: 2 }}
                    className="relative w-16 h-16 rounded-2xl flex items-center justify-center shadow-xl"
                    style={{ background: "var(--brand-primary)" }}
                  >
                    <Bot className="w-8 h-8 text-white" />
                  </motion.div>
                  {autoStatus === "running" && (
                    <>
                      <motion.div
                        animate={{ scale: [1, 1.55], opacity: [0.55, 0] }}
                        transition={{ repeat: Infinity, duration: 1.8 }}
                        className="absolute inset-0 rounded-2xl"
                        style={{ background: "var(--brand-primary)" }}
                      />
                      <motion.div
                        animate={{ scale: [1, 1.4], opacity: [0.3, 0] }}
                        transition={{ repeat: Infinity, duration: 1.8, delay: 0.4 }}
                        className="absolute inset-0 rounded-2xl"
                        style={{ background: "var(--brand-accent)" }}
                      />
                    </>
                  )}
                  {autoStatus === "success" && (
                    <motion.div
                      initial={{ scale: 0, rotate: -30 }}
                      animate={{ scale: 1, rotate: 0 }}
                      transition={{ type: "spring", stiffness: 360, damping: 18 }}
                      className="absolute -bottom-1 -left-1 w-7 h-7 rounded-full bg-emerald-500 border-2 border-card flex items-center justify-center"
                    >
                      <CheckCircle2 className="w-4 h-4 text-white" />
                    </motion.div>
                  )}
                </div>

                <h2 className="text-xl font-bold flex items-center gap-2">
                  <Sparkles className="w-4 h-4" style={{ color: "var(--brand-accent)" }} />
                  المساعد الذكي يتصل نيابةً عنك
                </h2>
                <p className="text-sm text-muted-foreground mt-1.5 leading-relaxed">
                  سأقوم بإعداد الاتصال ببيانات قاعدة <span className="font-bold text-foreground">Marketing2026</span> المحلية تلقائياً.
                </p>
              </div>

              {/* قائمة الخطوات */}
              <div className="relative z-10 space-y-2 mb-5">
                {AI_STEPS.map((step, idx) => {
                  const state: "done" | "active" | "pending" =
                    autoStatus === "success" || autoStep > idx
                      ? "done"
                      : autoStep === idx
                      ? "active"
                      : "pending";
                  const Icon = step.icon;
                  return (
                    <motion.div
                      key={step.key}
                      initial={{ opacity: 0, x: 12 }}
                      animate={{ opacity: 1, x: 0 }}
                      transition={{ delay: idx * 0.07 }}
                      className={cn(
                        "flex items-center gap-3 rounded-xl p-3 border transition-all",
                        state === "active" && "border-primary/50 bg-primary/8 shadow-sm",
                        state === "done" && "border-emerald-500/30 bg-emerald-500/5",
                        state === "pending" && "border-border/60 bg-muted/15 opacity-60",
                      )}
                    >
                      <div
                        className={cn(
                          "w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0 transition-colors",
                          state === "active" && "bg-primary text-primary-foreground",
                          state === "done" && "bg-emerald-500 text-white",
                          state === "pending" && "bg-muted text-muted-foreground",
                        )}
                      >
                        {state === "done" ? (
                          <CheckCircle2 className="w-4 h-4" />
                        ) : state === "active" ? (
                          <Loader2 className="w-4 h-4 animate-spin" />
                        ) : (
                          <Icon className="w-4 h-4" />
                        )}
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className={cn(
                          "text-sm font-medium leading-tight",
                          state === "pending" && "text-muted-foreground",
                        )}>{step.label}</p>
                        <p className="text-[11px] text-muted-foreground font-mono mt-0.5" dir="ltr">{step.detail}</p>
                      </div>
                      {state === "active" && (
                        <motion.div
                          animate={{ opacity: [0.3, 1, 0.3] }}
                          transition={{ repeat: Infinity, duration: 1.4 }}
                          className="w-1.5 h-1.5 rounded-full bg-primary mihbar-pulse"
                        />
                      )}
                    </motion.div>
                  );
                })}
              </div>

              {/* رسالة الحالة */}
              <AnimatePresence mode="wait">
                {autoStatus === "success" && (
                  <motion.div
                    key="success-msg"
                    initial={{ opacity: 0, y: 8 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0 }}
                    className="relative z-10 flex items-center gap-2.5 p-3 mb-4 text-sm bg-emerald-50 border border-emerald-200 rounded-xl dark:bg-emerald-950/30 dark:border-emerald-900/40"
                  >
                    <CheckCircle2 className="w-4 h-4 flex-shrink-0 text-emerald-600 dark:text-emerald-400" />
                    <span className="text-emerald-700 dark:text-emerald-300 font-semibold">
                      تم الاتصال بنجاح — يتم الانتقال للتطبيق...
                    </span>
                  </motion.div>
                )}
                {autoStatus === "error" && (
                  <motion.div
                    key="error-msg"
                    initial={{ opacity: 0, y: 8 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0 }}
                    className="relative z-10 flex items-start gap-2.5 p-3 mb-4 text-sm bg-red-50 border border-red-200 rounded-xl dark:bg-red-950/30 dark:border-red-900/40"
                  >
                    <XCircle className="w-4 h-4 flex-shrink-0 mt-0.5 text-red-600 dark:text-red-400" />
                    <div className="text-red-700 dark:text-red-300">
                      <p className="font-semibold">تعذّر الاتصال</p>
                      <p className="text-xs mt-0.5 text-red-600/80 dark:text-red-400/80">{autoError}</p>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>

              {/* أزرار الإجراءات */}
              <div className="relative z-10 flex gap-2.5">
                {autoStatus === "idle" && (
                  <>
                    <Button
                      type="button"
                      variant="outline"
                      className="flex-1 h-11"
                      onClick={() => setAutoOpen(false)}
                    >
                      إلغاء
                    </Button>
                    <Button
                      type="button"
                      className="flex-1 h-11 gap-2 font-bold shadow-md"
                      style={{ background: "var(--brand-primary)", color: "var(--fg-on-brand)" }}
                      onClick={runAutoConnect}
                    >
                      <Wand2 className="w-4 h-4" />
                      ابدأ الاتصال التلقائي
                    </Button>
                  </>
                )}
                {autoStatus === "running" && (
                  <Button type="button" disabled className="w-full h-11 gap-2">
                    <Loader2 className="w-4 h-4 animate-spin" />
                    جارٍ تنفيذ الخطوات…
                  </Button>
                )}
                {autoStatus === "error" && (
                  <>
                    <Button
                      type="button"
                      variant="outline"
                      className="flex-1 h-11"
                      onClick={() => setAutoOpen(false)}
                    >
                      إغلاق
                    </Button>
                    <Button
                      type="button"
                      className="flex-1 h-11 gap-2 font-bold"
                      style={{ background: "var(--brand-primary)", color: "var(--fg-on-brand)" }}
                      onClick={runAutoConnect}
                    >
                      <Zap className="w-4 h-4" />
                      إعادة المحاولة
                    </Button>
                  </>
                )}
                {autoStatus === "success" && (
                  <Button type="button" disabled className="w-full h-11 gap-2 bg-emerald-500 hover:bg-emerald-500 text-white">
                    <CheckCircle2 className="w-4 h-4" />
                    اكتمل بنجاح
                  </Button>
                )}
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
