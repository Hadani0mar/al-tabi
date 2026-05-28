import { cn } from "@/lib/utils";

type BadgeTone = "brand" | "success" | "warning" | "danger" | "info" | "neutral" | "accent";

const TONE_STYLES: Record<BadgeTone, { bg: string; fg: string; dot: string }> = {
  brand: {
    bg: "var(--brand-primary-soft)",
    fg: "var(--brand-primary-ink)",
    dot: "var(--brand-primary)",
  },
  success: {
    bg: "var(--success-soft)",
    fg: "var(--success-fg)",
    dot: "var(--success)",
  },
  warning: {
    bg: "var(--warning-soft)",
    fg: "var(--warning-fg)",
    dot: "var(--warning)",
  },
  danger: {
    bg: "var(--danger-soft)",
    fg: "var(--danger-fg)",
    dot: "var(--danger)",
  },
  info: {
    bg: "var(--info-soft)",
    fg: "var(--info-fg)",
    dot: "var(--info)",
  },
  neutral: {
    bg: "var(--bg-muted)",
    fg: "var(--fg-2)",
    dot: "var(--fg-muted)",
  },
  accent: {
    bg: "var(--brand-accent-soft)",
    fg: "var(--brand-accent-ink)",
    dot: "var(--brand-accent)",
  },
};

export function Badge({
  tone = "brand",
  children,
  dot = false,
  className,
}: {
  tone?: BadgeTone;
  children: React.ReactNode;
  dot?: boolean;
  className?: string;
}) {
  const c = TONE_STYLES[tone];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 whitespace-nowrap rounded-full px-2.5 py-0.5 text-[11.5px] font-semibold",
        className,
      )}
      style={{ background: c.bg, color: c.fg }}
    >
      {dot ? (
        <span className="h-1.5 w-1.5 rounded-full" style={{ background: c.dot }} />
      ) : null}
      {children}
    </span>
  );
}
