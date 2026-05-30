import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { FileX, ChevronLeft, Receipt } from "lucide-react";
import { CancelledInvoicesAddon } from "./cancelled-invoices-addon";
import { QuickSaleAddon } from "./quick-sale-addon";
import { cn } from "@/lib/utils";

type AddonsView = "menu" | "cancelled-invoices" | "quick-sale";

const SECTIONS: {
  id: Exclude<AddonsView, "menu">;
  title: string;
  description: string;
  icon: React.ReactNode;
  iconBg: string;
}[] = [
  {
    id: "quick-sale",
    title: "فاتورة سريعة",
    description: "مسح باركود — طباعة إيصال للزبون دون تسجيل في النظام",
    icon: <Receipt className="w-5 h-5" />,
    iconBg: "bg-emerald-500/15 text-emerald-600",
  },
  {
    id: "cancelled-invoices",
    title: "الفواتير الملغاة",
    description: "فواتير المبيعات والمشتريات الملغاة حسب التاريخ",
    icon: <FileX className="w-5 h-5" />,
    iconBg: "bg-red-500/15 text-red-600",
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

export function AddonsPage() {
  const [view, setView] = useState<AddonsView>("menu");

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
              <h1 className="text-2xl sm:text-3xl font-semibold">الإضافات</h1>
              <p className="text-sm text-muted-foreground mt-1">
                أدوات تشغيلية إضافية
              </p>
            </div>

            <nav className="flex-1 w-full rounded-xl border border-border bg-card overflow-hidden divide-y divide-border">
              {SECTIONS.map((section) => (
                <button
                  key={section.id}
                  type="button"
                  onClick={() => setView(section.id)}
                  className={cn(
                    "w-full flex items-center gap-4 px-4 py-4 sm:px-5 sm:py-[18px]",
                    "text-right transition-colors hover:bg-muted/50 active:bg-muted/70",
                  )}
                >
                  <div
                    className={cn(
                      "w-10 h-10 rounded-lg flex items-center justify-center shrink-0",
                      section.iconBg,
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
            {view === "quick-sale" && (
              <>
                <SectionHeader
                  title="فاتورة سريعة"
                  onBack={() => setView("menu")}
                />
                <div className="flex-1 space-y-5 overflow-y-auto min-h-0">
                  <QuickSaleAddon />
                </div>
              </>
            )}
            {view === "cancelled-invoices" && (
              <>
                <SectionHeader
                  title="الفواتير الملغاة"
                  onBack={() => setView("menu")}
                />
                <div className="flex-1 space-y-5 overflow-y-auto min-h-0">
                  <CancelledInvoicesAddon />
                </div>
              </>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
