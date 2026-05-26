import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { load } from "@tauri-apps/plugin-store";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Eye, EyeOff, Database, BarChart3,
  Server, Hash, User, Lock, ShieldCheck,
  Monitor, Save, Plug, Loader2,
  CheckCircle2, XCircle, AlertCircle,
  TrendingUp, FileBarChart2, Users, PieChart,
  KeyRound, Network, BookLock, Wifi,
  ChevronRight, ScrollText, ShieldAlert,
} from "lucide-react";
import { cn } from "@/lib/utils";

// ─── Types ─────────────────────────────────────────────────────
interface SqlConnection {
  server: string; port: number; database: string;
  username: string; password: string; use_windows_auth: boolean;
}
interface ConnectionResult {
  success: boolean; message: string; server_version: string | null;
}
const STORE_FILE = "connections.dat";

// ─── InputIcon wrapper ─────────────────────────────────────────
function FieldIcon({ icon: Icon, className }: { icon: React.ElementType; className?: string }) {
  return <Icon className={cn("absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground/60 pointer-events-none", className)} />;
}

// ─── Pupil ──────────────────────────────────────────────────────
interface PupilProps {
  size?: number; maxDistance?: number; pupilColor?: string;
  forceLookX?: number; forceLookY?: number;
}
const Pupil = ({ size = 12, maxDistance = 5, pupilColor = "#1a1a2e", forceLookX, forceLookY }: PupilProps) => {
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const move = (e: MouseEvent) => {
      if (!ref.current || (forceLookX !== undefined && forceLookY !== undefined)) return;
      const r = ref.current.getBoundingClientRect();
      const dx = e.clientX - (r.left + r.width / 2), dy = e.clientY - (r.top + r.height / 2);
      const d = Math.min(Math.sqrt(dx ** 2 + dy ** 2), maxDistance), a = Math.atan2(dy, dx);
      setPos({ x: Math.cos(a) * d, y: Math.sin(a) * d });
    };
    window.addEventListener("mousemove", move);
    return () => window.removeEventListener("mousemove", move);
  }, [maxDistance, forceLookX, forceLookY]);
  const px = forceLookX !== undefined ? forceLookX : pos.x;
  const py = forceLookY !== undefined ? forceLookY : pos.y;
  return <div ref={ref} className="rounded-full" style={{ width: size, height: size, backgroundColor: pupilColor, transform: `translate(${px}px,${py}px)`, transition: "transform 0.1s ease-out" }} />;
};

// ─── EyeBall ────────────────────────────────────────────────────
interface EyeBallProps {
  size?: number; pupilSize?: number; maxDistance?: number;
  eyeColor?: string; pupilColor?: string; isBlinking?: boolean;
  forceLookX?: number; forceLookY?: number;
}
const EyeBall = ({ size = 48, pupilSize = 16, maxDistance = 10, eyeColor = "white", pupilColor = "#1a1a2e", isBlinking = false, forceLookX, forceLookY }: EyeBallProps) => {
  const [pupilPos, setPupilPos] = useState({ x: 0, y: 0 });
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const move = (e: MouseEvent) => {
      if (!ref.current || forceLookX !== undefined) return;
      const r = ref.current.getBoundingClientRect();
      const dx = e.clientX - (r.left + r.width / 2), dy = e.clientY - (r.top + r.height / 2);
      const d = Math.min(Math.sqrt(dx ** 2 + dy ** 2), maxDistance), a = Math.atan2(dy, dx);
      setPupilPos({ x: Math.cos(a) * d, y: Math.sin(a) * d });
    };
    window.addEventListener("mousemove", move);
    return () => window.removeEventListener("mousemove", move);
  }, [maxDistance, forceLookX]);
  const px = forceLookX !== undefined ? forceLookX : pupilPos.x;
  const py = forceLookY !== undefined ? forceLookY : pupilPos.y;
  return (
    <div ref={ref} className="rounded-full flex items-center justify-center transition-all duration-150"
      style={{ width: size, height: isBlinking ? 2 : size, backgroundColor: eyeColor, overflow: "hidden" }}>
      {!isBlinking && <div className="rounded-full" style={{ width: pupilSize, height: pupilSize, backgroundColor: pupilColor, transform: `translate(${px}px,${py}px)`, transition: "transform 0.1s ease-out" }} />}
    </div>
  );
};

// ─── Main ───────────────────────────────────────────────────────
interface SqlLoginPageProps {
  onConnected?: (info: {
    server: string; port: number; database: string;
    username: string; password: string;
    use_windows_auth: boolean; server_version: string | null;
  }) => void;
}

export function SqlLoginPage({ onConnected }: SqlLoginPageProps) {
  const [form, setForm] = useState<SqlConnection>({ server: "", port: 1433, database: "", username: "", password: "", use_windows_auth: false });
  const [showPass, setShowPass] = useState(false);
  const [status, setStatus] = useState<"idle" | "loading" | "success" | "error">("idle");
  const [msg, setMsg] = useState("");
  const [version, setVersion] = useState<string | null>(null);
  const [savedConn, setSavedConn] = useState(false);
  const [rememberMe, setRememberMe] = useState(true);
  const [isTyping, setIsTyping] = useState(false);
  const [isPurpleBlink, setIsPurpleBlink] = useState(false);
  const [isBlackBlink, setIsBlackBlink] = useState(false);
  const [lookEachOther, setLookEachOther] = useState(false);
  const [purplePeek, setPurplePeek] = useState(false);
  const [mouse, setMouse] = useState({ x: 0, y: 0 });
  const purpleRef = useRef<HTMLDivElement>(null);
  const blackRef = useRef<HTMLDivElement>(null);
  const yellowRef = useRef<HTMLDivElement>(null);
  const orangeRef = useRef<HTMLDivElement>(null);

  useEffect(() => { const h = (e: MouseEvent) => setMouse({ x: e.clientX, y: e.clientY }); window.addEventListener("mousemove", h); return () => window.removeEventListener("mousemove", h); }, []);
  useEffect(() => { const s = () => { const t = setTimeout(() => { setIsPurpleBlink(true); setTimeout(() => { setIsPurpleBlink(false); s(); }, 150); }, Math.random() * 4000 + 3000); return t; }; const t = s(); return () => clearTimeout(t); }, []);
  useEffect(() => { const s = () => { const t = setTimeout(() => { setIsBlackBlink(true); setTimeout(() => { setIsBlackBlink(false); s(); }, 150); }, Math.random() * 4000 + 3000); return t; }; const t = s(); return () => clearTimeout(t); }, []);
  useEffect(() => { if (isTyping) { setLookEachOther(true); const t = setTimeout(() => setLookEachOther(false), 800); return () => clearTimeout(t); } else setLookEachOther(false); }, [isTyping]);
  useEffect(() => { if (form.password.length > 0 && showPass) { const t = setTimeout(() => { setPurplePeek(true); setTimeout(() => setPurplePeek(false), 800); }, Math.random() * 3000 + 2000); return () => clearTimeout(t); } else setPurplePeek(false); }, [form.password, showPass, purplePeek]);
  useEffect(() => { loadHistory(); }, []);

  async function loadHistory() {
    try {
      const store = await load(STORE_FILE, { autoSave: false, defaults: {} });
      const last = await store.get<string>("last_connection");
      const names = await store.get<string[]>("connection_names") ?? [];
      if (last && names.includes(last)) await loadConnection(last);
    } catch (_) {}
  }
  async function loadConnection(name: string) {
    try {
      const store = await load(STORE_FILE, { autoSave: false, defaults: {} });
      const enc = await store.get<Record<string, string>>(`conn_${name}`);
      if (!enc) return;
      const password = enc.password ? await invoke<string>("decrypt_value", { encrypted: enc.password }) : "";
      setForm({ server: enc.server ?? "", port: parseInt(enc.port ?? "1433"), database: enc.database ?? "", username: enc.username ?? "", password, use_windows_auth: enc.use_windows_auth === "true" });
      setSavedConn(true);
    } catch (_) {}
  }
  async function saveConnection(name: string) {
    try {
      const store = await load(STORE_FILE, { autoSave: false, defaults: {} });
      const encPassword = form.password ? await invoke<string>("encrypt_value", { value: form.password }) : "";
      await store.set(`conn_${name}`, { server: form.server, port: String(form.port), database: form.database, username: form.username, password: encPassword, use_windows_auth: String(form.use_windows_auth) });
      const names = await store.get<string[]>("connection_names") ?? [];
      if (!names.includes(name)) await store.set("connection_names", [...names, name]);
      await store.set("last_connection", name);
      await store.save();
      setSavedConn(true);
    } catch (e) { console.error(e); }
  }

  const calcPos = (ref: React.RefObject<HTMLDivElement | null>) => {
    if (!ref.current) return { faceX: 0, faceY: 0, bodySkew: 0 };
    const r = ref.current.getBoundingClientRect();
    const dx = mouse.x - (r.left + r.width / 2), dy = mouse.y - (r.top + r.height / 3);
    return { faceX: Math.max(-15, Math.min(15, dx / 20)), faceY: Math.max(-10, Math.min(10, dy / 30)), bodySkew: Math.max(-6, Math.min(6, -dx / 120)) };
  };
  const pp = calcPos(purpleRef), bp = calcPos(blackRef), yp = calcPos(yellowRef), op = calcPos(orangeRef);
  const hiding = form.password.length > 0 && !showPass;
  const visible = form.password.length > 0 && showPass;

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
            server_version: result.server_version,
          });
        }, 1200);
      } else { setStatus("error"); setMsg(result.message); }
    } catch (err) { setStatus("error"); setMsg(`خطأ: ${err}`); }
  }

  const set = (k: keyof SqlConnection, v: string | number | boolean) => setForm(p => ({ ...p, [k]: v }));

  // مميزات النظام للبانل الأيسر
  const features = [
    { icon: FileBarChart2, label: "تقارير ديناميكية", desc: "استعلامات مخزّنة في السحابة" },
    { icon: TrendingUp,    label: "تحليل فوري",       desc: "نتائج لحظية من SQL Server" },
    { icon: Users,         label: "متعدد المستخدمين", desc: "صلاحيات وتحكم كامل" },
    { icon: PieChart,      label: "مرئيات تفاعلية",   desc: "رسوم بيانية احترافية" },
  ];

  return (
    <div className="min-h-screen grid lg:grid-cols-2">

      {/* ══════════════ اليسار — البانل ══════════════ */}
      <div className="relative hidden lg:flex flex-col justify-between bg-gradient-to-br from-indigo-700 via-indigo-600 to-violet-700 p-12 text-white overflow-hidden">
        {/* خلفية شبكية */}
        <div className="absolute inset-0 bg-[linear-gradient(rgba(255,255,255,0.05)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,0.05)_1px,transparent_1px)] bg-[size:20px_20px]" />
        <div className="absolute top-1/4 right-1/4 w-64 h-64 bg-white/10 rounded-full blur-3xl" />
        <div className="absolute bottom-1/4 left-1/4 w-96 h-96 bg-white/5 rounded-full blur-3xl" />

        {/* ── شعار ── */}
        <div className="relative z-10 flex items-center gap-3 text-lg font-bold">
          <div className="w-9 h-9 rounded-xl bg-white/15 backdrop-blur flex items-center justify-center shadow-lg">
            <BarChart3 className="w-5 h-5" />
          </div>
          <div>
            <span className="block leading-tight">نظام التقارير</span>
            <span className="block text-xs font-normal text-white/50">Reports System v1.0</span>
          </div>
        </div>

        {/* ── الشخصيات ── */}
        <div className="relative z-10 flex items-end justify-center" style={{ height: 400 }}>
          <div className="relative" style={{ width: 520, height: 370 }}>
            {/* البنفسجي */}
            <div ref={purpleRef} className="absolute bottom-0 transition-all duration-700"
              style={{ left: 60, width: 170, height: hiding ? 430 : 380, backgroundColor: "#7C3AED", borderRadius: "10px 10px 0 0", zIndex: 1, transform: visible ? "skewX(0deg)" : hiding ? `skewX(${(pp.bodySkew || 0) - 12}deg) translateX(38px)` : `skewX(${pp.bodySkew || 0}deg)`, transformOrigin: "bottom center" }}>
              <div className="absolute flex gap-7 transition-all duration-700" style={{ left: visible ? 18 : lookEachOther ? 52 : 42 + pp.faceX, top: visible ? 32 : lookEachOther ? 62 : 38 + pp.faceY }}>
                <EyeBall size={17} pupilSize={6} maxDistance={4} eyeColor="white" pupilColor="#1a1a2e" isBlinking={isPurpleBlink} forceLookX={visible ? (purplePeek ? 4 : -4) : lookEachOther ? 3 : undefined} forceLookY={visible ? (purplePeek ? 5 : -4) : lookEachOther ? 4 : undefined} />
                <EyeBall size={17} pupilSize={6} maxDistance={4} eyeColor="white" pupilColor="#1a1a2e" isBlinking={isPurpleBlink} forceLookX={visible ? (purplePeek ? 4 : -4) : lookEachOther ? 3 : undefined} forceLookY={visible ? (purplePeek ? 5 : -4) : lookEachOther ? 4 : undefined} />
              </div>
            </div>
            {/* الأسود */}
            <div ref={blackRef} className="absolute bottom-0 transition-all duration-700"
              style={{ left: 225, width: 115, height: 300, backgroundColor: "#1e1b4b", borderRadius: "8px 8px 0 0", zIndex: 2, transform: visible ? "skewX(0deg)" : lookEachOther ? `skewX(${(bp.bodySkew || 0) * 1.5 + 10}deg) translateX(18px)` : hiding ? `skewX(${(bp.bodySkew || 0) * 1.5}deg)` : `skewX(${bp.bodySkew || 0}deg)`, transformOrigin: "bottom center" }}>
              <div className="absolute flex gap-5 transition-all duration-700" style={{ left: visible ? 8 : lookEachOther ? 30 : 24 + bp.faceX, top: visible ? 26 : lookEachOther ? 10 : 30 + bp.faceY }}>
                <EyeBall size={15} pupilSize={5} maxDistance={4} eyeColor="white" pupilColor="#1a1a2e" isBlinking={isBlackBlink} forceLookX={visible ? -4 : lookEachOther ? 0 : undefined} forceLookY={visible ? -4 : lookEachOther ? -4 : undefined} />
                <EyeBall size={15} pupilSize={5} maxDistance={4} eyeColor="white" pupilColor="#1a1a2e" isBlinking={isBlackBlink} forceLookX={visible ? -4 : lookEachOther ? 0 : undefined} forceLookY={visible ? -4 : lookEachOther ? -4 : undefined} />
              </div>
            </div>
            {/* البرتقالي */}
            <div ref={orangeRef} className="absolute bottom-0 transition-all duration-700"
              style={{ left: 0, width: 220, height: 185, backgroundColor: "#FB923C", borderRadius: "110px 110px 0 0", zIndex: 3, transform: visible ? "skewX(0deg)" : `skewX(${op.bodySkew || 0}deg)`, transformOrigin: "bottom center" }}>
              <div className="absolute flex gap-7 transition-all duration-200" style={{ left: visible ? 46 : 76 + (op.faceX || 0), top: visible ? 80 : 86 + (op.faceY || 0) }}>
                <Pupil size={11} maxDistance={4} pupilColor="#1a1a2e" forceLookX={visible ? -5 : undefined} forceLookY={visible ? -4 : undefined} />
                <Pupil size={11} maxDistance={4} pupilColor="#1a1a2e" forceLookX={visible ? -5 : undefined} forceLookY={visible ? -4 : undefined} />
              </div>
            </div>
            {/* الأصفر */}
            <div ref={yellowRef} className="absolute bottom-0 transition-all duration-700"
              style={{ left: 295, width: 135, height: 220, backgroundColor: "#FBBF24", borderRadius: "68px 68px 0 0", zIndex: 4, transform: visible ? "skewX(0deg)" : `skewX(${yp.bodySkew || 0}deg)`, transformOrigin: "bottom center" }}>
              <div className="absolute flex gap-5 transition-all duration-200" style={{ left: visible ? 18 : 49 + (yp.faceX || 0), top: visible ? 33 : 38 + (yp.faceY || 0) }}>
                <Pupil size={11} maxDistance={4} pupilColor="#1a1a2e" forceLookX={visible ? -5 : undefined} forceLookY={visible ? -4 : undefined} />
                <Pupil size={11} maxDistance={4} pupilColor="#1a1a2e" forceLookX={visible ? -5 : undefined} forceLookY={visible ? -4 : undefined} />
              </div>
              <div className="absolute w-16 h-1 bg-[#1a1a2e] rounded-full transition-all duration-200" style={{ left: visible ? 8 : 36 + (yp.faceX || 0), top: visible ? 84 : 84 + (yp.faceY || 0) }} />
            </div>
          </div>
        </div>

        {/* ── مميزات النظام ── */}
        <div className="relative z-10 grid grid-cols-2 gap-3 mb-4">
          {features.map(({ icon: Icon, label, desc }) => (
            <div key={label} className="flex items-start gap-2.5 bg-white/8 rounded-xl p-3 backdrop-blur-sm border border-white/10">
              <div className="w-7 h-7 rounded-lg bg-white/15 flex items-center justify-center flex-shrink-0 mt-0.5">
                <Icon className="w-3.5 h-3.5 text-white" />
              </div>
              <div>
                <p className="text-xs font-semibold text-white leading-tight">{label}</p>
                <p className="text-[10px] text-white/45 leading-tight mt-0.5">{desc}</p>
              </div>
            </div>
          ))}
        </div>

        {/* ── روابط ── */}
        <div className="relative z-10 flex items-center gap-5 text-xs text-white/40">
          <a href="#" className="flex items-center gap-1 hover:text-white/70 transition-colors">
            <ScrollText className="w-3 h-3" /> سياسة الخصوصية
          </a>
          <a href="#" className="flex items-center gap-1 hover:text-white/70 transition-colors">
            <BookLock className="w-3 h-3" /> شروط الاستخدام
          </a>
        </div>
      </div>

      {/* ══════════════ اليمين — النموذج ══════════════ */}
      <div className="flex items-center justify-center p-8 bg-background" dir="rtl">
        <div className="w-full max-w-[420px]">

          {/* موبايل شعار */}
          <div className="lg:hidden flex items-center justify-center gap-2 text-lg font-bold mb-10">
            <div className="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center">
              <BarChart3 className="w-4 h-4 text-primary" />
            </div>
            <span>نظام التقارير</span>
          </div>

          {/* ── العنوان ── */}
          <div className="text-center mb-8">
            <div className="inline-flex items-center justify-center w-14 h-14 rounded-2xl bg-primary/10 mb-4 shadow-inner">
              <Database className="w-7 h-7 text-primary" />
            </div>
            <h1 className="text-2xl font-bold tracking-tight mb-1">اتصال بقاعدة البيانات</h1>
            <p className="text-muted-foreground text-sm flex items-center justify-center gap-1.5">
              <Network className="w-3.5 h-3.5" />
              أدخل بيانات الاتصال بـ SQL Server
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
                    onFocus={() => setIsTyping(true)} onBlur={() => setIsTyping(false)}
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
                  onFocus={() => setIsTyping(true)} onBlur={() => setIsTyping(false)}
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
                      onFocus={() => setIsTyping(true)} onBlur={() => setIsTyping(false)}
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
              disabled={!form.server || !form.database || status === "loading"}>
              {status === "loading"
                ? <><Loader2 className="w-5 h-5 animate-spin" /> جارٍ الاتصال...</>
                : <><Plug className="w-5 h-5" /> اتصال واختبار</>}
            </Button>
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
    </div>
  );
}
