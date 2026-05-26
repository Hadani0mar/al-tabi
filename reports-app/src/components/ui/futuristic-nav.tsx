"use client";

import React, { useState } from "react";
import { motion } from "framer-motion";
import {
  FileBarChart2,
  Search,
  Bell,
  Bookmark,
  Settings,
} from "lucide-react";

interface NavItem {
  id: number;
  icon: React.ReactNode;
  label: string;
}

const items: NavItem[] = [
  { id: 0, icon: <FileBarChart2 size={22} />, label: "التقارير" },
  { id: 1, icon: <Search size={22} />,        label: "بحث"       },
  { id: 2, icon: <Bell size={22} />,          label: "تنبيهات"   },
  { id: 3, icon: <img src="/ai.svg" alt="AI" className="w-6 h-6 object-contain drop-shadow-md" />, label: "الذكاء" },
  { id: 4, icon: <Bookmark size={22} />,      label: "محفوظات"   },
  { id: 5, icon: <Settings size={22} />,      label: "إعدادات"   },
];

interface LumaBarProps {
  activeIndex?: number;
  onNavigate?: (index: number) => void;
}

const LumaBar = ({ activeIndex = 0, onNavigate }: LumaBarProps) => {
  const [active, setActive] = useState(activeIndex);

  const handleClick = (index: number) => {
    setActive(index);
    onNavigate?.(index);
  };

  return (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50">
      <div
        className="relative flex items-center justify-center gap-1 bg-white/20 dark:bg-black/20 backdrop-blur-2xl rounded-full px-4 py-2.5 shadow-2xl border border-gray-200/50 dark:border-gray-700/50 overflow-hidden"
        dir="ltr"
      >
        {/* Background glow that follows active item */}
        <motion.div
          className="absolute w-14 h-14 bg-gradient-to-r from-indigo-400 to-violet-500 rounded-full blur-2xl -z-10 opacity-70"
          animate={{ left: `calc(${(active / items.length) * 100}% + ${100 / items.length / 2}%)`, translateX: "-50%" }}
          transition={{ type: "spring", stiffness: 500, damping: 30 }}
        />

        {items.map((item, index) => {
          const isActive = index === active;
          return (
            <motion.div key={item.id} className="relative flex flex-col items-center group">
              {/* Tooltip — shown above */}
              <span className="absolute bottom-full mb-2 px-2.5 py-1 text-[11px] font-medium rounded-lg bg-gray-800 text-white dark:bg-gray-100 dark:text-gray-900 opacity-0 group-hover:opacity-100 transition-opacity whitespace-nowrap pointer-events-none shadow-lg">
                {item.label}
              </span>

              {/* Button */}
              <motion.button
                onClick={() => handleClick(index)}
                whileHover={{ scale: 1.15 }}
                whileTap={{ scale: 0.9 }}
                animate={{
                  scale: isActive ? 1.3 : 1,
                  color: isActive ? "#6366f1" : undefined,
                }}
                transition={{ type: "spring", stiffness: 400, damping: 20 }}
                className={`flex items-center justify-center w-12 h-12 rounded-full transition-colors duration-200 relative z-10 ${
                  isActive
                    ? "text-indigo-600 dark:text-indigo-400"
                    : "text-gray-500 dark:text-gray-400 hover:text-indigo-500 dark:hover:text-indigo-400"
                }`}
                aria-label={item.label}
              >
                {item.icon}

                {/* Active dot indicator */}
                {isActive && (
                  <motion.span
                    layoutId="active-dot"
                    className="absolute bottom-1 left-1/2 -translate-x-1/2 w-1 h-1 rounded-full bg-indigo-500"
                    transition={{ type: "spring", stiffness: 500, damping: 30 }}
                  />
                )}
              </motion.button>
            </motion.div>
          );
        })}
      </div>
    </div>
  );
};

export default LumaBar;
