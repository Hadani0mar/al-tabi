import { load } from "@tauri-apps/plugin-store";

export type ThemeId = "mihbar" | "mihbar-dark" | "default" | "elegant-luxury" | "cosmic-night";

export const THEME_OPTIONS: {
  id: ThemeId;
  title: string;
  description: string;
  iconBg: string;
}[] = [
  {
    id: "mihbar",
    title: "Mihbar — نهاري",
    description: "Daylight Ledger — teal & copper على ورق دافئ",
    iconBg: "bg-[#0F6E70]",
  },
  {
    id: "mihbar-dark",
    title: "Mihbar — ليلي",
    description: "Lamplit Ledger — نفس الهوية في الوضع الداكن",
    iconBg: "bg-[#121A2C]",
  },
  {
    id: "default",
    title: "الافتراضي (قديم)",
    description: "بنفسجي/indigo — النسخة السابقة",
    iconBg: "bg-gradient-to-br from-indigo-500 to-violet-600",
  },
  {
    id: "elegant-luxury",
    title: "Elegant Luxury",
    description: "أحمر burgundy وذهبي",
    iconBg: "bg-gradient-to-br from-red-800 to-amber-200",
  },
  {
    id: "cosmic-night",
    title: "Cosmic Night",
    description: "بنفسجي كوني",
    iconBg: "bg-gradient-to-br from-indigo-900 via-violet-700 to-purple-400",
  },
];

const LEGACY_THEME_CLASS: Record<Exclude<ThemeId, "mihbar" | "mihbar-dark">, string> = {
  "elegant-luxury": "theme-elegant-luxury",
  "cosmic-night": "theme-cosmic-night",
  default: "",
};

const STORE_KEY = "active_theme";
const LEGACY_STORE_KEY = "elegant_luxury_theme";

const VALID_THEMES = new Set<ThemeId>([
  "mihbar",
  "mihbar-dark",
  "default",
  "elegant-luxury",
  "cosmic-night",
]);

export function normalizeThemeId(value: unknown): ThemeId {
  if (typeof value === "string" && VALID_THEMES.has(value as ThemeId)) {
    return value as ThemeId;
  }
  return "mihbar";
}

function clearLegacyThemeClasses(root: HTMLElement) {
  Object.values(LEGACY_THEME_CLASS).forEach((cls) => {
    if (cls) root.classList.remove(cls);
  });
}

export function applyTheme(themeId: ThemeId) {
  const safeId = normalizeThemeId(themeId);
  const root = document.documentElement;
  clearLegacyThemeClasses(root);
  root.removeAttribute("data-theme");
  root.classList.remove("dark");

  if (safeId === "mihbar") {
    root.setAttribute("data-theme", "mihbar-light");
    return;
  }
  if (safeId === "mihbar-dark") {
    root.setAttribute("data-theme", "mihbar-dark");
    root.classList.add("dark");
    return;
  }
  if (safeId === "default") {
    return;
  }
  const cls = LEGACY_THEME_CLASS[safeId];
  if (cls) root.classList.add(cls);
}

export async function loadActiveTheme(): Promise<ThemeId> {
  try {
    const store = await load("settings.json");
    const saved = await store.get<string>(STORE_KEY);
    if (saved && VALID_THEMES.has(saved as ThemeId)) {
      return saved as ThemeId;
    }
    const legacy = await store.get<boolean>(LEGACY_STORE_KEY);
    if (legacy) return "elegant-luxury";
    return "mihbar";
  } catch {
    return "mihbar";
  }
}

export async function saveActiveTheme(themeId: ThemeId): Promise<void> {
  const store = await load("settings.json");
  await store.set(STORE_KEY, themeId);
  await store.delete(LEGACY_STORE_KEY);
  await store.save();
  applyTheme(themeId);
}
