import { useState, useRef, KeyboardEvent, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createClient } from "@supabase/supabase-js";
import {
  Play, Loader2, ShoppingCart,
  AlertCircle, CheckCircle2, Download, RotateCcw,
  Search, Tag,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ConnectionInfo } from "@/App";

const SUPABASE_URL = "https://nsgmhijtaaenpqxxgjds.supabase.co";
const SUPABASE_KEY = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Im5zZ21oaWp0YWFlbnBxeHhnamRzIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzkxODU1NTMsImV4cCI6MjA5NDc2MTU1M30.bva5PiwsoBiLR7u2upQV7q2spl6GhAg-JqrQ8nnUC8E";
const supabase    = createClient(SUPABASE_URL, SUPABASE_KEY);
const REPORT_NAME = "best_supplier_by_product";

interface QueryResult {
  columns: string[];
  rows: string[][];
  row_count: number;
}
interface Props { connInfo: ConnectionInfo }

function buildConn(c: ConnectionInfo) {
  return {
    server:           c.server,
    port:             c.port,
    database:         c.database,
    username:         c.username,
    password:         c.password ?? "",
    use_windows_auth: c.use_windows_auth,
    disable_encryption: c.disable_encryption ?? false,
  };
}

export function SupplierPricePage({ connInfo }: Props) {
  const [searchTerm, setSearchTerm] = useState("");
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [sugLoading,  setSugLoading]  = useState(false);
  const [showDrop,    setShowDrop]    = useState(false);
  const [focusedIdx,  setFocusedIdx]  = useState(-1);
  const inputRef  = useRef<HTMLInputElement>(null);
  const dropRef   = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [status,   setStatus]   = useState<"idle"|"loading"|"success"|"error">("idle");
  const [errorMsg, setErrorMsg] = useState("");
  const [result,   setResult]   = useState<QueryResult | null>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (dropRef.current && !dropRef.current.contains(e.target as Node) &&
          inputRef.current && !inputRef.current.contains(e.target as Node)) {
        setShowDrop(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const searchProducts = useCallback(async (q: string) => {
    if (q.trim().length < 2) { setSuggestions([]); setShowDrop(false); return; }
    setSugLoading(true);
    try {
      const names = await invoke<string[]>("search_products", {
        conn:  buildConn(connInfo),
        query: q,
      });
      setSuggestions(names);
      setShowDrop(names.length > 0);
      setFocusedIdx(-1);
    } catch (e) {
      console.error("search_products error:", e);
      setSuggestions([]);
    } finally {
      setSugLoading(false);
    }
  }, [connInfo]);

  const handleInputChange = (val: string) => {
    setSearchTerm(val);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => searchProducts(val), 280);
  };

  const selectSuggestion = (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) return;
    setSearchTerm(trimmed);
    setSuggestions([]);
    setShowDrop(false);
    inputRef.current?.focus();
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (showDrop && suggestions.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setFocusedIdx(i => Math.min(i + 1, suggestions.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setFocusedIdx(i => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        if (focusedIdx >= 0) {
          selectSuggestion(suggestions[focusedIdx]);
        } else {
          handleRun();
        }
        return;
      }
      if (e.key === "Escape") { setShowDrop(false); return; }
    } else if (e.key === "Enter") {
      handleRun();
    }
  };

  async function handleRun() {
    setShowDrop(false);
    if (!searchTerm.trim()) {
      setErrorMsg("الرجاء إدخال اسم أو كود المنتج للبحث");
      setStatus("error");
      return;
    }
    setStatus("loading"); setResult(null); setErrorMsg("");
    try {
      const { data, error } = await supabase
        .from("reports")
        .select("sql_query")
        .eq("name", REPORT_NAME)
        .single();

      if (error) throw new Error(`Supabase: ${error.message}`);
      if (!data?.sql_query) throw new Error("القالب غير موجود في Supabase");

      const res = await invoke<QueryResult>("execute_search_report", {
        conn: buildConn(connInfo),
        sqlTemplate: data.sql_query,
        searchTerm: searchTerm.trim(),
      });

      setResult(res);
      setStatus("success");
    } catch (err) {
      console.error("[SupplierPrice]", err);
      setErrorMsg(String(err));
      setStatus("error");
    }
  }

  const handleReset = () => {
    setSearchTerm(""); setResult(null);
    setStatus("idle"); setErrorMsg("");
    setSuggestions([]); setShowDrop(false);
  };

  const exportCSV = () => {
    if (!result) return;
    const bom = "﻿";
    const header = result.columns.join(",");
    const body   = result.rows.map(r => r.map(c => `"${c}"`).join(",")).join("\n");
    const blob = new Blob([bom + header + "\n" + body], { type: "text/csv;charset=utf-8" });
    const url  = URL.createObjectURL(blob);
    const a    = document.createElement("a");
    a.href = url; a.download = "product_last_price.csv"; a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="p-5 space-y-5 max-w-5xl mx-auto" dir="rtl">
      {/* ── رأس ── */}
      <div className="flex items-center justify-between pt-4">
        <div>
          <h1 className="text-xl font-bold flex items-center gap-2">
            <ShoppingCart className="w-5 h-5 text-primary" />
            معرفة آخر سعر شراء لمنتج
          </h1>
          <p className="text-xs text-muted-foreground mt-0.5">
            ابحث عن طريق كود المنتج أو الاسم لجلب آخر سعر ومورد
          </p>
        </div>
        {result && (
          <div className="flex gap-2">
            <Button size="sm" variant="outline" onClick={exportCSV} className="gap-1.5 text-xs">
              <Download className="w-3.5 h-3.5" /> CSV
            </Button>
            <Button size="sm" variant="ghost" onClick={handleReset} className="gap-1.5 text-xs">
              <RotateCcw className="w-3.5 h-3.5" /> مسح
            </Button>
          </div>
        )}
      </div>

      {/* ── إدخال البحث والاقتراحات ── */}
      <div className="flex items-start gap-2">
        <div className="relative flex-1">
          <div className="relative">
            {sugLoading
              ? <Loader2 className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground/50 animate-spin flex-shrink-0" />
              : <Search className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground/50" />
            }
            <input
              ref={inputRef}
              value={searchTerm}
              onChange={e => handleInputChange(e.target.value)}
              onKeyDown={handleKeyDown}
              onFocus={() => searchTerm.trim().length >= 2 && setShowDrop(suggestions.length > 0)}
              placeholder="أدخل كود المنتج أو جزء من الاسم هنا..."
              className="w-full h-12 pr-10 pl-4 rounded-xl border border-border bg-background outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary transition-all text-sm"
              dir="ltr"
              autoComplete="off"
            />
          </div>

          {/* Dropdown الاقتراحات */}
          {showDrop && suggestions.length > 0 && (
            <div
              ref={dropRef}
              className="absolute top-full right-0 left-0 mt-2 z-50 rounded-xl border border-border bg-popover shadow-xl overflow-hidden"
            >
              <div className="max-h-56 overflow-y-auto">
                {suggestions.map((s, i) => (
                  <button
                    key={s}
                    onMouseDown={e => { e.preventDefault(); selectSuggestion(s); }}
                    className={cn(
                      "w-full text-right px-4 py-2.5 text-sm flex items-center gap-2 transition-colors",
                      i === focusedIdx
                        ? "bg-primary text-primary-foreground"
                        : "hover:bg-muted"
                    )}
                    dir="ltr"
                  >
                    <Tag className="w-3 h-3 flex-shrink-0" />
                    <span className="flex-1 text-left">{s}</span>
                  </button>
                ))}
              </div>
              <div className="px-3 py-1.5 border-t border-border bg-muted/30 text-[10px] text-muted-foreground">
                {suggestions.length} نتيجة — اختر من القائمة أو تنقّل بالأسهم ↑↓
              </div>
            </div>
          )}
        </div>
        
        <Button onClick={handleRun} disabled={status === "loading" || !searchTerm.trim()} className="h-12 px-6 gap-2 shrink-0">
          {status === "loading" ? <Loader2 className="w-4 h-4 animate-spin" /> : <Play className="w-4 h-4" />}
          بحث
        </Button>
      </div>

      {/* ── خطأ ── */}
      {status === "error" && (
        <div className="flex items-start gap-2.5 p-3 text-sm text-red-600 bg-red-50 border border-red-200 rounded-xl dark:bg-red-950/20 dark:border-red-900/30 dark:text-red-400">
          <AlertCircle className="w-4 h-4 flex-shrink-0 mt-0.5" />
          <span>{errorMsg}</span>
        </div>
      )}

      {/* ── النتائج ── */}
      {status === "success" && result && (
        <div className="space-y-3">
          <div className="flex items-center gap-2 text-sm font-semibold text-emerald-600 dark:text-emerald-400">
            <CheckCircle2 className="w-4 h-4" />
            {result.row_count} نتيجة
          </div>

          {result.row_count === 0 ? (
            <div className="text-center py-12 text-muted-foreground text-sm border border-dashed border-border rounded-xl">
              لا توجد نتائج تطابق بحثك.
            </div>
          ) : (
            <div className="rounded-xl border border-border overflow-hidden shadow-sm">
              <div className="overflow-x-auto">
                <table className="w-full text-sm text-right">
                  <thead>
                    <tr className="bg-muted/80 border-b border-border">
                      {result.columns.map(col => (
                         <th key={col} className="px-4 py-3 font-bold text-muted-foreground text-xs whitespace-nowrap bg-primary/5">
                           {col}
                         </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {result.rows.map((row, ri) => (
                      <tr key={ri} className={cn(
                        "border-b border-border/50 hover:bg-muted/30 transition-colors",
                        ri % 2 === 0 ? "bg-background" : "bg-muted/10"
                      )}>
                        {row.map((cell, ci) => (
                          <td key={ci} className="px-4 py-3 whitespace-nowrap font-medium" dir={ci === 0 ? "ltr" : "rtl"}>
                            {result.columns[ci].includes("سعر") ? (
                               <span className="font-bold text-emerald-600 dark:text-emerald-400 text-base">{cell}</span>
                            ) : result.columns[ci].includes("تاريخ") ? (
                               <span className="font-mono text-xs text-muted-foreground">{cell}</span>
                            ) : cell || <span className="text-muted-foreground/30 text-xs">—</span>}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
