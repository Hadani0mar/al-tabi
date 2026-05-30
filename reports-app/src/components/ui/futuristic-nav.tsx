"use client";

import {
  FileBarChart2,
  Bookmark,
  Settings,
  Sparkles,
  Puzzle,
} from "lucide-react";
import { cn } from "@/lib/utils";

const items = [
  { id: 0, icon: <FileBarChart2 size={16} />, label: "التقارير" },
  { id: 1, icon: <Bookmark size={16} />, label: "المحفوظات" },
  { id: 2, icon: <Sparkles size={16} />, label: "الذكاء", accent: true },
  { id: 3, icon: <Puzzle size={16} />, label: "الإضافات" },
  { id: 4, icon: <Settings size={16} />, label: "الإعدادات" },
] as const;

interface MihbarNavProps {
  activeIndex?: number;
  onNavigate?: (index: number) => void;
}

export default function MihbarNav({ activeIndex = 2, onNavigate }: MihbarNavProps) {
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-[18px] z-50 flex justify-center">
      <nav
        className="pointer-events-auto inline-flex gap-1 rounded-full p-1.5"
        style={{
          background: "var(--bg-elevated)",
          border: "1px solid var(--border-default)",
          boxShadow: "var(--shadow-lg)",
        }}
        dir="ltr"
        aria-label="التنقل الرئيسي"
      >
        {items.map((item) => {
          const isActive = item.id === activeIndex;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onNavigate?.(item.id)}
              className={cn(
                "inline-flex items-center gap-1.5 rounded-full border-none px-3.5 py-2 text-[12.5px] font-semibold transition-all",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2",
              )}
              style={{
                fontFamily: "var(--font-ui)",
                color: isActive ? "var(--fg-on-brand)" : "var(--fg-2)",
                background: isActive ? "var(--brand-primary)" : "transparent",
                boxShadow: isActive
                  ? "0 4px 12px rgba(15,110,112,0.30), inset 0 1px 0 rgba(255,255,255,0.16)"
                  : "none",
                transitionDuration: "var(--dur-base)",
              }}
              onMouseEnter={(e) => {
                if (!isActive) e.currentTarget.style.background = "var(--bg-subtle)";
              }}
              onMouseLeave={(e) => {
                if (!isActive) e.currentTarget.style.background = "transparent";
              }}
            >
              {item.icon}
              {item.label}
            </button>
          );
        })}
      </nav>
    </div>
  );
}
