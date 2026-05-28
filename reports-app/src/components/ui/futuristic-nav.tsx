"use client";

import {
  FileBarChart2,
  Search,
  Bell,
  Bookmark,
  Settings,
  Sparkles,
} from "lucide-react";
import { cn } from "@/lib/utils";

interface NavItem {
  id: number;
  icon: React.ReactNode;
  label: string;
  accent?: boolean;
}

const items: NavItem[] = [
  { id: 0, icon: <FileBarChart2 size={16} />, label: "التقارير" },
  { id: 1, icon: <Search size={16} />, label: "بحث" },
  { id: 2, icon: <Bell size={16} />, label: "تنبيهات" },
  { id: 3, icon: <Sparkles size={16} />, label: "الذكاء", accent: true },
  { id: 4, icon: <Bookmark size={16} />, label: "محفوظات" },
  { id: 5, icon: <Settings size={16} />, label: "إعدادات" },
];

interface MihbarNavProps {
  activeIndex?: number;
  onNavigate?: (index: number) => void;
}

export default function MihbarNav({ activeIndex = 0, onNavigate }: MihbarNavProps) {
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
