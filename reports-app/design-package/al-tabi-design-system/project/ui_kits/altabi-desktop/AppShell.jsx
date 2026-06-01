/* global React, Icon, UI */
// AppShell — desktop window chrome, title bar, and bottom navigation pill.

function TitleBar({ businessName, connected }) {
  return (
    <div style={{
      height: 40,
      minHeight: 40,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: '0 14px',
      background: 'var(--bg-canvas)',
      borderBottom: '1px solid var(--border-subtle)',
      userSelect: 'none',
      flexShrink: 0,
    }}>
      {/* Window controls — kept on visual-left because Windows convention */}
      <div style={{ display: 'flex', gap: 6 }}>
        {['close','maximize','minimize'].map((k) => (
          <button key={k} aria-label={k} style={{
            width: 12, height: 12, borderRadius: '50%',
            border: 'none', cursor: 'pointer', padding: 0,
            background: k === 'close' ? '#E07268' : k === 'maximize' ? '#DFA94C' : '#9CA3AF',
            opacity: 0.85,
          }}/>
        ))}
      </div>

      <div style={{
        display: 'flex', alignItems: 'center', gap: 8,
        fontFamily: 'var(--font-display)',
        fontWeight: 600, fontSize: 13.5,
        color: 'var(--fg-1)',
        whiteSpace: 'nowrap',
      }}>
        <img src="../../assets/logo-mark.svg" alt="" width={18} height={18} style={{ borderRadius: 4, display: 'block' }} />
        التابي
        <span style={{ fontFamily: 'var(--font-ui)', fontWeight: 400, fontSize: 12, color: 'var(--fg-3)' }}>
          · {businessName}
        </span>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <UI.Badge tone={connected ? 'success' : 'neutral'} dot>
          {connected ? 'متصل' : 'غير متصل'}
        </UI.Badge>
      </div>
    </div>
  );
}

const TABS = [
  { id: 'reports',  label: 'التقارير',   Icon: Icon.Calendar },
  { id: 'search',   label: 'بحث',         Icon: Icon.Search },
  { id: 'alerts',   label: 'تنبيهات',    Icon: Icon.Bell },
  { id: 'ai',       label: 'الذكاء',     Icon: Icon.Sparkles, accent: true },
  { id: 'saved',    label: 'المحفوظات', Icon: Icon.Bookmark },
  { id: 'settings', label: 'الإعدادات', Icon: Icon.Settings },
];

function BottomNav({ active, onChange }) {
  return (
    <div style={{
      position: 'absolute',
      bottom: 18,
      insetInlineStart: 0,
      insetInlineEnd: 0,
      display: 'flex',
      justifyContent: 'center',
      pointerEvents: 'none',
    }}>
      <nav style={{
        display: 'inline-flex',
        gap: 4,
        padding: 6,
        background: 'var(--bg-elevated)',
        border: '1px solid var(--border-default)',
        borderRadius: 'var(--radius-pill)',
        boxShadow: 'var(--shadow-lg)',
        pointerEvents: 'auto',
      }}>
        {TABS.map((tab) => {
          const isActive = active === tab.id;
          return (
            <button
              key={tab.id}
              onClick={() => onChange(tab.id)}
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 6,
                padding: '8px 14px',
                borderRadius: 'var(--radius-pill)',
                fontSize: 12.5,
                fontWeight: 600,
                color: isActive ? 'var(--fg-on-brand)' : 'var(--fg-2)',
                background: isActive ? 'var(--brand-primary)' : 'transparent',
                boxShadow: isActive ? '0 4px 12px rgba(15,110,112,0.30), inset 0 1px 0 rgba(255,255,255,0.16)' : 'none',
                border: 'none',
                cursor: 'pointer',
                fontFamily: 'var(--font-ui)',
                transition: 'all var(--dur-base) var(--ease-in-out)',
              }}
              onMouseEnter={(e) => { if (!isActive) e.currentTarget.style.background = 'var(--bg-subtle)'; }}
              onMouseLeave={(e) => { if (!isActive) e.currentTarget.style.background = 'transparent'; }}
            >
              <tab.Icon size={16} />
              {tab.label}
            </button>
          );
        })}
      </nav>
    </div>
  );
}

function AppShell({ children, active, onChangeTab, businessName, connected, showNav = true }) {
  return (
    <div style={{
      width: 1180,
      height: 720,
      background: 'var(--bg-canvas)',
      borderRadius: 14,
      overflow: 'hidden',
      boxShadow: 'var(--shadow-xl)',
      border: '1px solid var(--border-default)',
      display: 'flex',
      flexDirection: 'column',
      position: 'relative',
      fontFamily: 'var(--font-ui)',
    }}>
      <TitleBar businessName={businessName} connected={connected} />
      <main style={{
        flex: 1,
        overflow: 'hidden',
        position: 'relative',
        background: 'var(--bg-canvas)',
      }}>
        {children}
      </main>
      {showNav && <BottomNav active={active} onChange={onChangeTab} />}
    </div>
  );
}

window.AppShell = AppShell;
