import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ChevronLeft,
  ChevronRight,
  CalendarDays,
  FileX,
  Loader2,
  RefreshCw,
  ShoppingCart,
  Truck,
} from "lucide-react";
import { Button } from "./button";
import { cn } from "@/lib/utils";

export interface CancelledInvoiceRow {
  invoice_id: number;
  invoice_kind: string;
  invoice_kind_label: string;
  invoice_time: string;
  updated_at: string | null;
  party_name: string;
  employee_name: string;
  note: string;
}

function toIsoDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function parseIsoDate(iso: string): Date {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, m - 1, d);
}

function addDays(iso: string, delta: number): string {
  const d = parseIsoDate(iso);
  d.setDate(d.getDate() + delta);
  return toIsoDate(d);
}

function isToday(iso: string): boolean {
  return iso === toIsoDate(new Date());
}

function formatArabicDate(iso: string): string {
  return parseIsoDate(iso).toLocaleDateString("ar-LY", {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

function formatTime(isoDateTime: string): string {
  const t = isoDateTime.includes("T")
    ? isoDateTime
    : isoDateTime.replace(" ", "T");
  const d = new Date(t);
  if (Number.isNaN(d.getTime())) return isoDateTime;
  return d.toLocaleTimeString("ar-LY", { hour: "2-digit", minute: "2-digit" });
}

export function CancelledInvoicesAddon() {
  const [targetDate, setTargetDate] = useState(() => toIsoDate(new Date()));
  const [rows, setRows] = useState<CancelledInvoiceRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (date: string) => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<CancelledInvoiceRow[]>("list_cancelled_invoices", {
        targetDate: date,
      });
      setRows(data);
    } catch (e) {
      console.error(e);
      setError(String(e));
      setRows([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load(targetDate);
  }, [targetDate, load]);

  const salesCount = useMemo(
    () => rows.filter((r) => r.invoice_kind === "sale").length,
    [rows],
  );
  const purchaseCount = useMemo(
    () => rows.filter((r) => r.invoice_kind === "purchase").length,
    [rows],
  );

  return (
    <div className="flex flex-col gap-5">
      {/* Date navigator */}
      <div
        className="rounded-2xl border p-4"
        style={{
          borderColor: "var(--border-default)",
          background: "var(--bg-elevated)",
          boxShadow: "var(--shadow-sm)",
        }}
      >
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-1">
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="h-9 w-9 shrink-0 rounded-xl"
              onClick={() => setTargetDate((d) => addDays(d, -1))}
              aria-label="اليوم السابق"
            >
              <ChevronRight className="h-4 w-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="h-9 w-9 shrink-0 rounded-xl"
              onClick={() => setTargetDate((d) => addDays(d, 1))}
              disabled={isToday(targetDate)}
              aria-label="اليوم التالي"
            >
              <ChevronLeft className="h-4 w-4" />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="rounded-xl mr-1"
              disabled={isToday(targetDate)}
              onClick={() => setTargetDate(toIsoDate(new Date()))}
            >
              اليوم
            </Button>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <label
              htmlFor="cancelled-date"
              className="inline-flex items-center gap-1.5 rounded-xl border px-3 py-2 text-sm cursor-pointer"
              style={{
                borderColor: "var(--border-default)",
                background: "var(--bg-surface)",
              }}
            >
              <CalendarDays className="h-4 w-4 text-muted-foreground" />
              <input
                id="cancelled-date"
                type="date"
                value={targetDate}
                onChange={(e) => e.target.value && setTargetDate(e.target.value)}
                className="bg-transparent border-none outline-none text-sm font-medium"
                dir="ltr"
              />
            </label>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="h-9 w-9 rounded-xl"
              onClick={() => load(targetDate)}
              disabled={loading}
              aria-label="تحديث"
            >
              <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
            </Button>
          </div>
        </div>

        <p className="mt-3 text-sm font-medium text-foreground">{formatArabicDate(targetDate)}</p>
        {!isToday(targetDate) && (
          <p className="text-[11px] text-muted-foreground mt-0.5">تاريخ محدّد — ليس اليوم الحالي</p>
        )}
      </div>

      {/* Summary chips */}
      <div className="flex flex-wrap gap-2">
        <span
          className="inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-semibold"
          style={{ background: "var(--brand-primary-soft)", color: "var(--brand-primary-ink)" }}
        >
          <ShoppingCart className="h-3.5 w-3.5" />
          مبيعات ملغاة: {salesCount}
        </span>
        <span
          className="inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-semibold"
          style={{ background: "var(--warning-soft)", color: "var(--warning-fg)" }}
        >
          <Truck className="h-3.5 w-3.5" />
          مشتريات ملغاة: {purchaseCount}
        </span>
        <span
          className="inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-xs font-semibold"
          style={{ background: "var(--bg-subtle)", color: "var(--fg-2)" }}
        >
          الإجمالي: {rows.length}
        </span>
      </div>

      {/* Content */}
      {loading ? (
        <div className="flex flex-col items-center justify-center gap-3 py-16 text-muted-foreground">
          <Loader2 className="h-8 w-8 animate-spin" style={{ color: "var(--brand-primary)" }} />
          <span className="text-sm">جارٍ تحميل الفواتير الملغاة...</span>
        </div>
      ) : error ? (
        <div
          className="rounded-2xl border p-5 text-center"
          style={{ borderColor: "var(--danger)", background: "var(--danger-soft)" }}
        >
          <p className="text-sm font-medium" style={{ color: "var(--danger-fg)" }}>
            {error}
          </p>
          <Button type="button" variant="outline" size="sm" className="mt-3 rounded-xl" onClick={() => load(targetDate)}>
            إعادة المحاولة
          </Button>
        </div>
      ) : rows.length === 0 ? (
        <div
          className="flex flex-col items-center justify-center rounded-2xl border border-dashed py-14 px-6 text-center"
          style={{
            borderColor: "var(--border-default)",
            background: "var(--bg-elevated)",
          }}
        >
          <FileX className="h-10 w-10 mb-3" style={{ color: "var(--fg-3)" }} strokeWidth={1.5} />
          <p className="text-sm font-medium text-foreground">لا توجد فواتير ملغاة في هذا التاريخ</p>
          <p className="mt-2 text-xs text-muted-foreground max-w-xs leading-relaxed">
            جرّب يوماً سابقاً باستخدام الأسهم أو اختر تاريخاً من التقويم.
          </p>
        </div>
      ) : (
        <div
          className="overflow-hidden rounded-2xl border"
          style={{
            borderColor: "var(--border-default)",
            background: "var(--bg-elevated)",
            boxShadow: "var(--shadow-sm)",
          }}
        >
          <div className="overflow-x-auto">
            <table className="w-full min-w-[640px] text-sm" dir="rtl">
              <thead>
                <tr style={{ background: "var(--bg-subtle)" }}>
                  {["النوع", "رقم الفاتورة", "الوقت", "الطرف", "الموظف", "ملاحظة"].map((h) => (
                    <th
                      key={h}
                      className="px-4 py-3 text-right text-[11px] font-semibold uppercase tracking-wide"
                      style={{ color: "var(--fg-2)" }}
                    >
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr
                    key={`${row.invoice_kind}-${row.invoice_id}`}
                    className="border-t transition-colors hover:bg-muted/40"
                    style={{ borderColor: "var(--border-subtle)" }}
                  >
                    <td className="px-4 py-3">
                      <span
                        className={cn(
                          "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-semibold",
                        )}
                        style={
                          row.invoice_kind === "sale"
                            ? {
                                background: "var(--brand-primary-soft)",
                                color: "var(--brand-primary-ink)",
                              }
                            : {
                                background: "var(--warning-soft)",
                                color: "var(--warning-fg)",
                              }
                        }
                      >
                        {row.invoice_kind === "sale" ? (
                          <ShoppingCart className="h-3 w-3" />
                        ) : (
                          <Truck className="h-3 w-3" />
                        )}
                        {row.invoice_kind_label}
                      </span>
                    </td>
                    <td className="px-4 py-3 font-mono text-xs" dir="ltr">
                      #{row.invoice_id}
                    </td>
                    <td className="px-4 py-3 text-xs whitespace-nowrap" dir="ltr">
                      {formatTime(row.invoice_time)}
                    </td>
                    <td className="px-4 py-3 max-w-[140px] truncate" title={row.party_name}>
                      {row.party_name}
                    </td>
                    <td className="px-4 py-3 max-w-[120px] truncate" title={row.employee_name}>
                      {row.employee_name}
                    </td>
                    <td className="px-4 py-3 max-w-[160px] truncate text-muted-foreground" title={row.note}>
                      {row.note}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
