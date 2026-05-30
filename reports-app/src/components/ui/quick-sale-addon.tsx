import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import {
  Barcode,
  Info,
  Loader2,
  Minus,
  Plus,
  Printer,
  Search,
  ShoppingCart,
  Trash2,
} from "lucide-react";
import { Button } from "./button";
import { Input } from "./input";
import { Label } from "./label";
import { cn } from "@/lib/utils";

export interface PosProduct {
  bar_id: number;
  item_id: number;
  item_name: string;
  item_model: string;
  barcode: string;
  unit_id: number;
  unit_desc: string;
  unit_qty: number;
  price: number;
  last_cost: number;
  aver_cost: number;
  public_price: number;
  stock_qty: number;
}

interface CartLine {
  key: string;
  product: PosProduct;
  qty: number;
  price: number;
}

interface SavedReceipt {
  custName: string;
  note: string;
  lines: { name: string; unit: string; qty: number; price: number }[];
}

function productLabel(p: PosProduct): string {
  const code = p.item_model.trim();
  const unit = p.unit_desc.trim();
  const parts = [p.item_name.trim()];
  if (code) parts.push(`(${code})`);
  if (unit) parts.push(`— ${unit}`);
  if (p.barcode.trim()) parts.push(`[${p.barcode.trim()}]`);
  return parts.join(" ");
}

function productReceiptName(p: PosProduct): string {
  return p.item_name.trim();
}

function formatMoney(n: number): string {
  return n.toLocaleString("ar-LY", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

export function QuickSaleAddon() {
  const [query, setQuery] = useState("");
  const [suggestions, setSuggestions] = useState<PosProduct[]>([]);
  const [sugLoading, setSugLoading] = useState(false);
  const [showDrop, setShowDrop] = useState(false);
  const [focusedIdx, setFocusedIdx] = useState(-1);
  const [dropStyle, setDropStyle] = useState<CSSProperties>({});

  const [cart, setCart] = useState<CartLine[]>([]);
  const [custName, setCustName] = useState("زبون نقدي");
  const [note, setNote] = useState("");

  const [printing, setPrinting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [lastReceipt, setLastReceipt] = useState<SavedReceipt | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  const dropRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (
        dropRef.current &&
        !dropRef.current.contains(e.target as Node) &&
        inputRef.current &&
        !inputRef.current.contains(e.target as Node)
      ) {
        setShowDrop(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const updateDropPosition = useCallback(() => {
    const el = inputRef.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setDropStyle({
      position: "fixed",
      top: r.bottom + 6,
      left: r.left,
      width: r.width,
      zIndex: 10000,
    });
  }, []);

  useLayoutEffect(() => {
    if (!showDrop || suggestions.length === 0) return;
    updateDropPosition();
    window.addEventListener("scroll", updateDropPosition, true);
    window.addEventListener("resize", updateDropPosition);
    return () => {
      window.removeEventListener("scroll", updateDropPosition, true);
      window.removeEventListener("resize", updateDropPosition);
    };
  }, [showDrop, suggestions.length, sugLoading, updateDropPosition]);

  const addProduct = useCallback((product: PosProduct, qty = 1) => {
    const key = `${product.item_id}-${product.bar_id}-${product.unit_id}`;
    setCart((prev) => {
      const idx = prev.findIndex((l) => l.key === key);
      if (idx >= 0) {
        const next = [...prev];
        next[idx] = { ...next[idx], qty: next[idx].qty + qty };
        return next;
      }
      return [
        ...prev,
        {
          key,
          product,
          qty,
          price: product.price > 0 ? product.price : product.public_price,
        },
      ];
    });
    setQuery("");
    setSuggestions([]);
    setShowDrop(false);
    setFocusedIdx(-1);
    setError(null);
    requestAnimationFrame(() => inputRef.current?.focus());
  }, []);

  const fetchProducts = useCallback(
    async (q: string) => {
      const trimmed = q.trim();
      if (trimmed.length < 1) {
        setSuggestions([]);
        setShowDrop(false);
        return;
      }
      setSugLoading(true);
      try {
        const hits = await invoke<PosProduct[]>("search_pos_products", { query: trimmed });
        setSuggestions(hits);
        setShowDrop(hits.length > 0);
        setFocusedIdx(-1);

        if (
          hits.length === 1 &&
          trimmed === hits[0].barcode.trim() &&
          /^\d{6,}$/.test(trimmed)
        ) {
          addProduct(hits[0]);
        }
      } catch (e) {
        console.error(e);
        setSuggestions([]);
        setShowDrop(false);
      } finally {
        setSugLoading(false);
      }
    },
    [addProduct],
  );

  const handleQueryChange = (val: string) => {
    setQuery(val);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => fetchProducts(val), 220);
  };

  const handleQueryKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setShowDrop(true);
      setFocusedIdx((i) => Math.min(i + 1, suggestions.length - 1));
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setFocusedIdx((i) => Math.max(i - 1, 0));
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (focusedIdx >= 0 && suggestions[focusedIdx]) {
        addProduct(suggestions[focusedIdx]);
        return;
      }
      if (suggestions.length === 1) {
        addProduct(suggestions[0]);
        return;
      }
      if (query.trim()) fetchProducts(query.trim());
      return;
    }
    if (e.key === "Escape") {
      setShowDrop(false);
    }
  };

  const updateLineQty = (key: string, delta: number) => {
    setCart((prev) =>
      prev
        .map((l) => (l.key === key ? { ...l, qty: Math.max(0, l.qty + delta) } : l))
        .filter((l) => l.qty > 0),
    );
  };

  const updateLinePrice = (key: string, price: number) => {
    setCart((prev) =>
      prev.map((l) => (l.key === key ? { ...l, price: Math.max(0, price) } : l)),
    );
  };

  const removeLine = (key: string) => {
    setCart((prev) => prev.filter((l) => l.key !== key));
  };

  const total = useMemo(
    () => cart.reduce((sum, l) => sum + l.qty * l.price, 0),
    [cart],
  );

  const buildReceiptLines = (items: CartLine[]) =>
    items.map((l) => ({
      name: productReceiptName(l.product),
      unit: l.product.unit_desc.trim() || "—",
      qty: l.qty,
      price: l.price,
    }));

  const openPdf = async (path: string) => {
    await invoke("open_local_file", { path });
  };

  const printReceipt = async (payload: SavedReceipt) => {
    const path = await invoke<string>("print_pos_receipt", {
      custName: payload.custName,
      note: payload.note.trim() || null,
      lines: payload.lines,
    });
    await openPdf(path);
  };

  const handlePrint = async () => {
    if (cart.length === 0) {
      setError("أضف أصنافاً قبل الطباعة.");
      return;
    }
    setPrinting(true);
    setError(null);
    setSuccess(null);
    try {
      const payload: SavedReceipt = {
        custName: custName.trim() || "زبون نقدي",
        note: note.trim(),
        lines: buildReceiptLines(cart),
      };
      await printReceipt(payload);
      setLastReceipt(payload);
      setSuccess(`تم طباعة إثبات البيع — الإجمالي ${formatMoney(total)}`);
      setCart([]);
      setNote("");
      inputRef.current?.focus();
    } catch (e) {
      setError(String(e));
    } finally {
      setPrinting(false);
    }
  };

  const handleReprintLast = async () => {
    if (!lastReceipt) return;
    setPrinting(true);
    setError(null);
    try {
      await printReceipt(lastReceipt);
    } catch (e) {
      setError(String(e));
    } finally {
      setPrinting(false);
    }
  };

  return (
    <div className="flex flex-col gap-5 pb-8">
      <div
        className="rounded-xl border px-4 py-3 text-sm flex gap-2 items-start"
        style={{
          borderColor: "var(--border-default)",
          background: "var(--bg-subtle)",
          color: "var(--text-muted)",
        }}
      >
        <Info className="h-4 w-4 shrink-0 mt-0.5" />
        <div dir="rtl" className="leading-relaxed space-y-1.5 min-w-0 flex-1">
          <p className="font-medium text-foreground">إثبات بيع للعميل فقط</p>
          <ul className="space-y-1 text-[13px] leading-6">
            <li>• يُطبَع إيصال ورقي للتسليم للزبون</li>
            <li>• لا يُحفظ في نظام المبيعات</li>
            <li>• لا يُخصَم من المخزون</li>
            <li>• لا يُسجَّل في المحاسبة</li>
          </ul>
        </div>
      </div>

      <div
        className="rounded-2xl border p-4 space-y-3"
        style={{
          borderColor: "var(--border-default)",
          background: "var(--bg-elevated)",
          boxShadow: "var(--shadow-sm)",
        }}
      >
        <div className="flex items-center gap-2 text-sm font-medium text-foreground">
          <Barcode className="h-4 w-4 text-muted-foreground" />
          مسح الباركود أو ابحث باسم المنتج
        </div>
        <div className="relative">
          <Search className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
          <Input
            ref={inputRef}
            value={query}
            onChange={(e) => handleQueryChange(e.target.value)}
            onKeyDown={handleQueryKeyDown}
            onFocus={() => suggestions.length > 0 && setShowDrop(true)}
            placeholder="امسح الباركود أو اكتب اسم/كود المنتج..."
            className="pr-10 text-base h-11 rounded-xl"
            autoComplete="off"
            dir="auto"
          />
          {sugLoading && (
            <Loader2 className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 animate-spin text-muted-foreground" />
          )}
        </div>
        {showDrop &&
          suggestions.length > 0 &&
          createPortal(
            <div
              ref={dropRef}
              role="listbox"
              style={{
                ...dropStyle,
                background: "hsl(var(--card))",
                border: "1px solid hsl(var(--border))",
                boxShadow: "0 10px 40px rgba(0,0,0,0.18)",
              }}
              className="max-h-72 overflow-y-auto rounded-xl"
            >
              {suggestions.map((hit, i) => (
                <button
                  key={`${hit.bar_id}-${hit.item_id}`}
                  type="button"
                  role="option"
                  aria-selected={focusedIdx === i}
                  className={cn(
                    "w-full text-right px-3 py-2.5 text-sm border-b border-border last:border-0",
                    "hover:bg-muted active:bg-muted/80",
                    focusedIdx === i && "bg-muted",
                  )}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    addProduct(hit);
                  }}
                >
                  <div className="font-medium leading-snug">{productLabel(hit)}</div>
                  <div className="flex flex-wrap gap-2 mt-1 text-xs text-muted-foreground">
                    <span>السعر: {formatMoney(hit.price > 0 ? hit.price : hit.public_price)}</span>
                    <span>المخزون: {hit.stock_qty.toLocaleString("ar-LY")}</span>
                  </div>
                </button>
              ))}
            </div>,
            document.body,
          )}
        <p className="text-[11px] text-muted-foreground leading-relaxed">
          امسح الباركود بالماسح الضوئي ثم Enter — أو اختر من القائمة. Enter يضيف الصنف للسلة.
        </p>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <div className="grid gap-2">
          <Label htmlFor="pos-cust">العميل</Label>
          <Input
            id="pos-cust"
            value={custName}
            onChange={(e) => setCustName(e.target.value)}
            className="rounded-xl"
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor="pos-note">ملاحظة (اختياري)</Label>
          <Input
            id="pos-note"
            value={note}
            onChange={(e) => setNote(e.target.value)}
            className="rounded-xl"
            placeholder="ملاحظة على الإيصال"
          />
        </div>
      </div>

      <div
        className="rounded-2xl border overflow-hidden"
        style={{
          borderColor: "var(--border-default)",
          background: "var(--bg-elevated)",
        }}
      >
        <div
          className="flex items-center justify-between px-4 py-3 border-b"
          style={{ borderColor: "var(--border-subtle)", background: "var(--bg-subtle)" }}
        >
          <div className="flex items-center gap-2 font-semibold text-sm">
            <ShoppingCart className="h-4 w-4" />
            سلة البيع
            <span className="text-muted-foreground font-normal">({cart.length})</span>
          </div>
          <span className="text-sm font-bold tabular-nums">{formatMoney(total)}</span>
        </div>

        {cart.length === 0 ? (
          <div className="py-12 text-center text-sm text-muted-foreground">
            لا توجد أصناف — امسح باركوداً أو ابحث عن منتج
          </div>
        ) : (
          <div className="divide-y divide-border">
            {cart.map((line) => (
              <div key={line.key} className="px-4 py-3 flex flex-col sm:flex-row sm:items-center gap-3">
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">{productReceiptName(line.product)}</p>
                  {line.product.barcode && (
                    <p className="text-[11px] text-muted-foreground font-mono" dir="ltr">
                      {line.product.barcode}
                    </p>
                  )}
                </div>
                <div className="flex flex-wrap items-center gap-2 shrink-0">
                  <div className="flex items-center rounded-lg border border-border overflow-hidden">
                    <button
                      type="button"
                      className="h-8 w-8 flex items-center justify-center hover:bg-muted"
                      onClick={() => updateLineQty(line.key, -1)}
                      aria-label="تقليل"
                    >
                      <Minus className="h-3.5 w-3.5" />
                    </button>
                    <span className="w-10 text-center text-sm tabular-nums">{line.qty}</span>
                    <button
                      type="button"
                      className="h-8 w-8 flex items-center justify-center hover:bg-muted"
                      onClick={() => updateLineQty(line.key, 1)}
                      aria-label="زيادة"
                    >
                      <Plus className="h-3.5 w-3.5" />
                    </button>
                  </div>
                  <Input
                    type="number"
                    min={0}
                    step={0.01}
                    value={line.price}
                    onChange={(e) => updateLinePrice(line.key, parseFloat(e.target.value) || 0)}
                    className="w-24 h-8 text-sm rounded-lg"
                    dir="ltr"
                  />
                  <span className="text-sm font-semibold tabular-nums w-20 text-left" dir="ltr">
                    {formatMoney(line.qty * line.price)}
                  </span>
                  <button
                    type="button"
                    className="h-8 w-8 flex items-center justify-center rounded-lg text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                    onClick={() => removeLine(line.key)}
                    aria-label="حذف"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {error && (
        <div
          className="rounded-xl border px-4 py-3 text-sm"
          style={{ borderColor: "var(--danger)", background: "var(--danger-soft)", color: "var(--danger-fg)" }}
        >
          {error}
        </div>
      )}
      {success && (
        <div className="rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-800 dark:text-emerald-200">
          {success}
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          className="rounded-xl"
          disabled={printing || cart.length === 0}
          onClick={handlePrint}
        >
          {printing ? (
            <Loader2 className="h-4 w-4 animate-spin ml-2" />
          ) : (
            <Printer className="h-4 w-4 ml-2" />
          )}
          طباعة إثبات البيع
        </Button>
        {lastReceipt && (
          <Button
            type="button"
            variant="outline"
            className="rounded-xl"
            disabled={printing}
            onClick={handleReprintLast}
          >
            <Printer className="h-4 w-4 ml-2" />
            إعادة طباعة آخر إيصال
          </Button>
        )}
      </div>
    </div>
  );
}
