import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import {
  Bookmark, Play, Trash2, X, Loader2,
  AlertTriangle, Search, RefreshCw, Database,
  CheckCircle2, XCircle, FileX,
  ChevronDown, Clock, Lock,
} from "lucide-react";
import { Button } from "./button";
import { Input } from "./input";
import { cn } from "@/lib/utils";

interface FavoriteDto {
  id: string;
  name: string;
  description: string;
  created_at_unix: number;
}

interface QueryResult {
  columns: string[];
  rows: string[][];
  row_count: number;
}

function formatRelativeAr(unix: number): string {
  const diff = Math.floor(Date.now() / 1000) - unix;
  if (diff < 60) return "الآن";
  if (diff < 3600) return `قبل ${Math.floor(diff / 60)} دقيقة`;
  if (diff < 86400) return `قبل ${Math.floor(diff / 3600)} ساعة`;
  if (diff < 2592000) return `قبل ${Math.floor(diff / 86400)} يوم`;
  if (diff < 31536000) return `قبل ${Math.floor(diff / 2592000)} شهر`;
  return `قبل ${Math.floor(diff / 31536000)} سنة`;
}

export function SavedQueriesPage() {
  const [favorites, setFavorites] = useState<FavoriteDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");

  const [runningId, setRunningId] = useState<string | null>(null);
  const [resultModal, setResultModal] = useState<{
    name: string;
    result: QueryResult | null;
    error: string | null;
  } | null>(null);

  const [deleteCandidate, setDeleteCandidate] = useState<FavoriteDto | null>(null);
  const [deleting, setDeleting] = useState(false);

  async function refresh() {
    setLoading(true);
    try {
      const list = await invoke<FavoriteDto[]>("list_favorite_queries");
      setFavorites(list);
    } catch (err) {
      console.error("Failed to load favorites:", err);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return favorites;
    return favorites.filter(f =>
      f.name.toLowerCase().includes(q) ||
      f.description.toLowerCase().includes(q),
    );
  }, [favorites, search]);

  async function runQuery(fav: FavoriteDto) {
    setRunningId(fav.id);
    setResultModal({ name: fav.name, result: null, error: null });
    try {
      const result = await invoke<QueryResult>("execute_favorite_query", { id: fav.id });
      setResultModal({ name: fav.name, result, error: null });
    } catch (err) {
      setResultModal({ name: fav.name, result: null, error: String(err) });
    } finally {
      setRunningId(null);
    }
  }

  async function confirmDelete() {
    if (!deleteCandidate) return;
    setDeleting(true);
    try {
      await invoke<boolean>("delete_favorite_query", { id: deleteCandidate.id });
      setFavorites(prev => prev.filter(f => f.id !== deleteCandidate.id));
      setDeleteCandidate(null);
    } catch (err) {
      console.error("Delete failed:", err);
      alert("تعذّر الحذف: " + err);
    } finally {
      setDeleting(false);
    }
  }

  return (
    <div className="min-h-screen p-6 pb-32" dir="rtl" style={{ background: "var(--bg-canvas)" }}>
      {/* العنوان */}
      <div className="flex items-center justify-between mb-6 pt-4">
        <div className="flex items-start gap-3">
          <div
            className="w-12 h-12 rounded-2xl flex items-center justify-center"
            style={{
              background: "var(--brand-primary)",
              color: "var(--fg-on-brand)",
              boxShadow: "var(--shadow-md)",
            }}
          >
            <Bookmark className="w-6 h-6" />
          </div>
          <div>
            <h1
              className="text-2xl font-bold leading-tight"
              style={{ fontFamily: "var(--font-display)", color: "var(--fg-1)" }}
            >
              المحفوظات
            </h1>
            <p className="text-sm mt-0.5" style={{ color: "var(--fg-2)" }}>
              الاستعلامات التي طلبتَ من الوكيل حفظها — جاهزة للتشغيل الفوري دون الحاجة للذكاء الاصطناعي.
            </p>
          </div>
        </div>
        <Button
          variant="outline"
          onClick={refresh}
          disabled={loading}
          className="gap-2 shrink-0"
          style={{ borderColor: "var(--border-default)", color: "var(--fg-2)" }}
        >
          {loading ? <Loader2 className="w-4 h-4 animate-spin" /> : <RefreshCw className="w-4 h-4" />}
          تحديث
        </Button>
      </div>

      {/* البحث */}
      {favorites.length > 0 && (
        <div className="relative mb-5 max-w-md">
          <Search className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground/60 pointer-events-none" />
          <Input
            placeholder="ابحث بالاسم أو الوصف أو محتوى SQL..."
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="pr-9"
          />
        </div>
      )}

      {/* المحتوى */}
      {loading ? (
        <div className="flex items-center justify-center py-20" style={{ color: "var(--fg-3)" }}>
          <Loader2 className="w-6 h-6 animate-spin" style={{ color: "var(--brand-primary)" }} />
        </div>
      ) : favorites.length === 0 ? (
        <EmptyState />
      ) : filtered.length === 0 ? (
        <div className="text-center py-16" style={{ color: "var(--fg-3)" }}>
          <FileX className="w-10 h-10 mx-auto mb-3 opacity-40" />
          <p>لا توجد نتائج مطابقة لـ "{search}".</p>
        </div>
      ) : (
        <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
          <AnimatePresence>
            {filtered.map((fav, idx) => (
              <motion.div
                key={fav.id}
                layout
                initial={{ opacity: 0, y: 12 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.95 }}
                transition={{ delay: idx * 0.03, type: "spring", stiffness: 240, damping: 22 }}
                className="group relative rounded-2xl p-4 transition-all overflow-hidden flex flex-col border hover:shadow-md"
                style={{
                  background: "var(--bg-surface)",
                  borderColor: "var(--border-default)",
                  boxShadow: "var(--shadow-xs)",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.borderColor = "color-mix(in srgb, var(--brand-primary) 40%, transparent)";
                  e.currentTarget.style.boxShadow = "var(--shadow-md)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.borderColor = "var(--border-default)";
                  e.currentTarget.style.boxShadow = "var(--shadow-xs)";
                }}
              >
                <div
                  className="absolute inset-x-0 top-0 h-0.5 opacity-0 group-hover:opacity-100 transition-opacity"
                  style={{ background: "var(--brand-primary)" }}
                />

                <div className="flex items-start gap-3 mb-3">
                  <div
                    className="w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0 border"
                    style={{
                      background: "var(--brand-primary-soft)",
                      borderColor: "color-mix(in srgb, var(--brand-primary) 20%, transparent)",
                      color: "var(--brand-primary)",
                    }}
                  >
                    <Bookmark className="w-5 h-5" />
                  </div>
                  <div className="flex-1 min-w-0">
                    <h3
                      className="text-sm font-bold leading-tight line-clamp-2"
                      style={{ color: "var(--fg-1)" }}
                      title={fav.name}
                    >
                      {fav.name}
                    </h3>
                    <div className="flex items-center gap-1 text-[11px] mt-1" style={{ color: "var(--fg-3)" }}>
                      <Clock className="w-3 h-3" />
                      <span>{formatRelativeAr(fav.created_at_unix)}</span>
                    </div>
                  </div>
                </div>

                {fav.description && (
                  <p className="text-xs leading-relaxed mb-3 line-clamp-3" style={{ color: "var(--fg-2)" }}>
                    {fav.description}
                  </p>
                )}

                <div className="flex-1 mb-3">
                  <div
                    className="rounded-lg border border-dashed p-3 flex items-center gap-2 text-[11px]"
                    style={{
                      borderColor: "var(--border-subtle)",
                      background: "color-mix(in srgb, var(--bg-subtle) 50%, transparent)",
                      color: "var(--fg-3)",
                    }}
                  >
                    <Lock className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "var(--brand-primary)" }} />
                    <span>الاستعلام محميّ — لا يُعرض المحتوى للحفاظ على الخصوصية.</span>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  <Button
                    size="sm"
                    className="flex-1 gap-1.5 font-bold shadow-sm h-9 border-0 hover:opacity-90"
                    style={{ background: "var(--brand-primary)", color: "var(--fg-on-brand)" }}
                    onClick={() => runQuery(fav)}
                    disabled={runningId === fav.id}
                  >
                    {runningId === fav.id ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    ) : (
                      <Play className="w-3.5 h-3.5 fill-current" />
                    )}
                    {runningId === fav.id ? "جارٍ التنفيذ" : "تشغيل"}
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    className="gap-1 h-9 px-2.5"
                    style={{
                      borderColor: "color-mix(in srgb, var(--danger) 25%, transparent)",
                      color: "var(--danger)",
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.background = "var(--danger-soft)";
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.background = "transparent";
                    }}
                    onClick={() => setDeleteCandidate(fav)}
                    title="حذف"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </Button>
                </div>
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}

      {/* ── Modal النتيجة ── */}
      <AnimatePresence>
        {resultModal && (
          <Overlay onClose={() => setResultModal(null)}>
            <ModalCard onClose={() => setResultModal(null)} maxWidth="max-w-5xl">
              <div className="flex items-start gap-3 mb-4">
                <div
                  className="w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0"
                  style={{
                    background: resultModal.error
                      ? "var(--danger-soft)"
                      : resultModal.result
                        ? "var(--success-soft)"
                        : "var(--brand-accent-soft)",
                    color: resultModal.error
                      ? "var(--danger)"
                      : resultModal.result
                        ? "var(--success)"
                        : "var(--brand-accent)",
                  }}
                >
                  {resultModal.error ? (
                    <XCircle className="w-5 h-5" />
                  ) : resultModal.result ? (
                    <CheckCircle2 className="w-5 h-5" />
                  ) : (
                    <Loader2 className="w-5 h-5 animate-spin" />
                  )}
                </div>
                <div className="flex-1 min-w-0">
                  <h3 className="text-lg font-bold leading-tight truncate" style={{ color: "var(--fg-1)" }}>
                    {resultModal.name}
                  </h3>
                  <p className="text-xs mt-0.5" style={{ color: "var(--fg-3)" }}>
                    {resultModal.error
                      ? "فشل تنفيذ الاستعلام"
                      : resultModal.result
                        ? `${resultModal.result.row_count} صف · ${resultModal.result.columns.length} عمود`
                        : "جارٍ تنفيذ الاستعلام..."}
                  </p>
                </div>
              </div>

              {resultModal.error && (
                <div
                  className="rounded-xl border p-4 text-sm font-mono whitespace-pre-wrap"
                  style={{
                    borderColor: "color-mix(in srgb, var(--danger) 25%, transparent)",
                    background: "var(--danger-soft)",
                    color: "var(--danger-fg)",
                  }}
                  dir="ltr"
                >
                  {resultModal.error}
                </div>
              )}

              {!resultModal.result && !resultModal.error && (
                <div className="flex items-center justify-center py-12" style={{ color: "var(--fg-3)" }}>
                  <Loader2 className="w-6 h-6 animate-spin" style={{ color: "var(--brand-primary)" }} />
                </div>
              )}

              {resultModal.result && resultModal.result.row_count === 0 && (
                <div
                  className="rounded-xl border p-8 text-center"
                  style={{
                    borderColor: "var(--border-subtle)",
                    background: "var(--bg-subtle)",
                    color: "var(--fg-3)",
                  }}
                >
                  <Database className="w-10 h-10 mx-auto mb-2 opacity-40" />
                  <p>لم يُرجع الاستعلام أي صفوف.</p>
                </div>
              )}

              {resultModal.result && resultModal.result.row_count > 0 && (
                <ResultTable result={resultModal.result} />
              )}

              <div className="flex justify-end mt-4">
                <Button variant="outline" onClick={() => setResultModal(null)}>
                  إغلاق
                </Button>
              </div>
            </ModalCard>
          </Overlay>
        )}
      </AnimatePresence>

      {/* ── Modal تأكيد الحذف ── */}
      <AnimatePresence>
        {deleteCandidate && (
          <Overlay onClose={() => !deleting && setDeleteCandidate(null)}>
            <ModalCard onClose={() => setDeleteCandidate(null)} maxWidth="max-w-sm">
              <div className="flex flex-col items-center text-center">
                <motion.div
                  initial={{ scale: 0, rotate: -30 }}
                  animate={{ scale: 1, rotate: 0 }}
                  transition={{ type: "spring", stiffness: 320, damping: 18 }}
                  className="w-14 h-14 rounded-2xl bg-red-500/15 flex items-center justify-center mb-4"
                >
                  <AlertTriangle className="w-7 h-7 text-red-500" />
                </motion.div>

                <h3 className="text-lg font-bold mb-1.5">حذف الاستعلام</h3>
                <p className="text-sm text-muted-foreground leading-relaxed mb-1">
                  هل أنت متأكد من حذف:
                </p>
                <p className="text-sm font-bold text-foreground mb-6 line-clamp-2">
                  «{deleteCandidate.name}»
                </p>

                <div className="flex gap-2.5 w-full">
                  <Button
                    variant="outline"
                    className="flex-1 h-10"
                    onClick={() => setDeleteCandidate(null)}
                    disabled={deleting}
                  >
                    إلغاء
                  </Button>
                  <Button
                    className="flex-1 h-10 gap-2 bg-red-500 hover:bg-red-600 text-white"
                    onClick={confirmDelete}
                    disabled={deleting}
                  >
                    {deleting ? (
                      <Loader2 className="w-4 h-4 animate-spin" />
                    ) : (
                      <Trash2 className="w-4 h-4" />
                    )}
                    {deleting ? "جارٍ الحذف..." : "حذف"}
                  </Button>
                </div>
              </div>
            </ModalCard>
          </Overlay>
        )}
      </AnimatePresence>
    </div>
  );
}

// ─── المساعدات ───────────────────────────────────────────

function EmptyState() {
  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex flex-col items-center justify-center py-24 text-center"
    >
      <div className="relative w-20 h-20 mb-5">
        <div className="absolute inset-0 rounded-3xl bg-gradient-to-br from-amber-400/20 to-orange-500/10 blur-2xl" />
        <div className="relative w-full h-full rounded-3xl bg-gradient-to-br from-amber-400/15 to-orange-500/10 border border-amber-500/15 flex items-center justify-center">
          <Bookmark className="w-9 h-9 text-amber-500/70" />
        </div>
      </div>
      <h3 className="text-lg font-bold mb-2">لا توجد استعلامات محفوظة بعد</h3>
      <p className="text-sm text-muted-foreground max-w-md leading-relaxed">
        اطلب من الوكيل الذكي حفظ أي استعلام ناجح عبر عبارات مثل «احفظ هذا الاستعلام» أو «خزّنه في المفضلة» —
        وسيظهر هنا تلقائياً، جاهزاً للتشغيل بضغطة زر.
      </p>
    </motion.div>
  );
}

function Overlay({ children, onClose }: { children: React.ReactNode; onClose: () => void }) {
  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.2 }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
      onClick={onClose}
    >
      {children}
    </motion.div>
  );
}

function ModalCard({
  children,
  onClose,
  maxWidth,
}: {
  children: React.ReactNode;
  onClose: () => void;
  maxWidth: string;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.92, y: 16 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.95 }}
      transition={{ type: "spring", stiffness: 280, damping: 24 }}
      className={cn(
        "relative w-full rounded-2xl bg-card border border-border shadow-2xl p-6 max-h-[90vh] overflow-auto",
        maxWidth,
      )}
      dir="rtl"
      onClick={e => e.stopPropagation()}
    >
      <button
        onClick={onClose}
        className="absolute left-4 top-4 w-8 h-8 rounded-lg flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-colors z-10"
      >
        <X className="w-4 h-4" />
      </button>
      {children}
    </motion.div>
  );
}

function ResultTable({ result }: { result: QueryResult }) {
  const [expanded, setExpanded] = useState(false);
  const visibleRows = expanded ? result.rows : result.rows.slice(0, 50);
  const hasMore = result.rows.length > 50;

  return (
    <div className="space-y-2">
      <div className="rounded-xl border border-border overflow-auto max-h-[55vh]" dir="rtl">
        <table className="w-full text-sm text-right border-collapse">
          <thead className="bg-muted/70 sticky top-0 z-10">
            <tr>
              {result.columns.map((col, i) => (
                <th
                  key={i}
                  className="px-4 py-2.5 font-bold text-[12px] uppercase tracking-wide border-b-2 border-border whitespace-nowrap text-right"
                >
                  {col}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {visibleRows.map((row, ri) => (
              <tr key={ri} className="hover:bg-muted/30 transition-colors">
                {row.map((cell, ci) => (
                  <td
                    key={ci}
                    className="px-4 py-2 border-b border-border/40 last:border-0 align-middle leading-relaxed text-[13px]"
                    title={cell}
                  >
                    {cell.length > 80 ? cell.substring(0, 80) + "…" : cell}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {hasMore && (
        <Button
          variant="outline"
          size="sm"
          className="w-full gap-2"
          onClick={() => setExpanded(e => !e)}
        >
          <ChevronDown className={cn("w-4 h-4 transition-transform", expanded && "rotate-180")} />
          {expanded ? "إخفاء" : `عرض كل الصفوف (${result.rows.length})`}
        </Button>
      )}
    </div>
  );
}
