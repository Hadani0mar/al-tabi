import { Badge } from "@/components/ui/mihbar-badge";

interface AppShellHeaderProps {
  businessName?: string;
  connected?: boolean;
}

export function AppShellHeader({ businessName, connected = true }: AppShellHeaderProps) {
  return (
    <header
      className="flex h-10 shrink-0 items-center justify-between border-b px-4"
      style={{
        background: "var(--bg-canvas)",
        borderColor: "var(--border-subtle)",
      }}
    >
      <div className="flex items-center gap-2">
        <img src="/assets/logo-mark.svg" alt="" width={20} height={20} className="rounded" />
        <span
          className="text-[13.5px] font-semibold"
          style={{ fontFamily: "var(--font-display)", color: "var(--fg-1)" }}
        >
          التابي
        </span>
        {businessName ? (
          <span className="text-xs" style={{ color: "var(--fg-3)" }}>
            · {businessName}
          </span>
        ) : null}
      </div>
      <Badge tone={connected ? "success" : "neutral"} dot>
        {connected ? "متصل" : "غير متصل"}
      </Badge>
    </header>
  );
}
