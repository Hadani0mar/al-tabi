/* global React, Icon */
// Shared low-level UI primitives. Buttons, inputs, badges, cards.

const cls = (...xs) => xs.filter(Boolean).join(' ');

function Button({ variant = 'primary', size = 'md', icon, children, onClick, type, disabled, style }) {
  const heights = { sm: 30, md: 38, lg: 46 };
  const pads    = { sm: '0 12px', md: '0 16px', lg: '0 22px' };
  const fs      = { sm: 13, md: 14, lg: 15 };
  const palette = {
    primary: { bg: 'var(--brand-primary)', fg: 'var(--fg-on-brand)', border: 'transparent', hoverBg: 'var(--brand-primary-hover)' },
    accent:  { bg: 'var(--brand-accent)',  fg: 'var(--fg-on-brand)', border: 'transparent', hoverBg: 'var(--brand-accent-hover)' },
    secondary: { bg: 'var(--bg-surface)', fg: 'var(--fg-1)', border: 'var(--border-default)', hoverBg: 'var(--bg-subtle)' },
    ghost:   { bg: 'transparent', fg: 'var(--brand-primary)', border: 'transparent', hoverBg: 'var(--brand-primary-soft)' },
    danger:  { bg: 'var(--danger)', fg: '#fff', border: 'transparent', hoverBg: 'var(--danger-fg)' },
  };
  const c = palette[variant] || palette.primary;
  const [hover, setHover] = React.useState(false);
  return (
    <button
      type={type || 'button'}
      onClick={onClick}
      disabled={disabled}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        fontFamily: 'var(--font-ui)',
        fontSize: fs[size],
        fontWeight: 600,
        height: heights[size],
        padding: pads[size],
        borderRadius: 'var(--radius-sm)',
        border: `1px solid ${c.border}`,
        background: hover && !disabled ? c.hoverBg : c.bg,
        color: c.fg,
        display: 'inline-flex',
        alignItems: 'center',
        gap: 8,
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.55 : 1,
        transition: 'background var(--dur-fast) var(--ease-soft)',
        ...style,
      }}
    >
      {icon}
      {children}
    </button>
  );
}

function Field({ label, hint, error, ok, children }) {
  return (
    <label style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
      {label && <span style={{ fontSize: 12, fontWeight: 600, color: 'var(--fg-2)' }}>{label}</span>}
      {children}
      {(hint || error || ok) && (
        <span style={{
          fontSize: 11.5,
          color: error ? 'var(--danger)' : ok ? 'var(--success)' : 'var(--fg-3)',
        }}>{error || ok || hint}</span>
      )}
    </label>
  );
}

function Input({ value, onChange, placeholder, type = 'text', status }) {
  const [focused, setFocused] = React.useState(false);
  const statusBorder = status === 'error' ? 'var(--danger)' : status === 'ok' ? 'var(--success)' : null;
  return (
    <input
      type={type}
      value={value}
      onChange={(e) => onChange && onChange(e.target.value)}
      placeholder={placeholder}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      style={{
        fontFamily: 'var(--font-ui)',
        fontSize: 14,
        padding: '0 12px',
        height: 40,
        background: 'var(--bg-elevated)',
        border: `1px solid ${statusBorder || (focused ? 'var(--border-focus)' : 'var(--border-default)')}`,
        borderRadius: 'var(--radius-sm)',
        color: 'var(--fg-1)',
        outline: 'none',
        boxShadow: focused ? 'var(--shadow-focus)' : 'none',
        transition: 'border-color var(--dur-fast), box-shadow var(--dur-fast)',
        width: '100%',
        boxSizing: 'border-box',
      }}
    />
  );
}

function Badge({ tone = 'brand', children, dot = false }) {
  const tones = {
    brand:   { bg: 'var(--brand-primary-soft)', fg: 'var(--brand-primary-ink)', dotBg: 'var(--brand-primary)' },
    success: { bg: 'var(--success-soft)', fg: 'var(--success-fg)', dotBg: 'var(--success)' },
    warning: { bg: 'var(--warning-soft)', fg: 'var(--warning-fg)', dotBg: 'var(--warning)' },
    danger:  { bg: 'var(--danger-soft)',  fg: 'var(--danger-fg)',  dotBg: 'var(--danger)' },
    info:    { bg: 'var(--info-soft)',    fg: 'var(--info-fg)',    dotBg: 'var(--info)' },
    neutral: { bg: 'var(--bg-muted)',     fg: 'var(--fg-2)',       dotBg: 'var(--fg-muted)' },
    accent:  { bg: 'var(--brand-accent-soft)', fg: 'var(--brand-accent-ink)', dotBg: 'var(--brand-accent)' },
  };
  const c = tones[tone] || tones.brand;
  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center', gap: 6,
      fontSize: 11.5, fontWeight: 600,
      padding: '3px 10px',
      borderRadius: 'var(--radius-pill)',
      background: c.bg, color: c.fg,
      whiteSpace: 'nowrap',
    }}>
      {dot && <span style={{ width: 6, height: 6, borderRadius: '50%', background: c.dotBg }}></span>}
      {children}
    </span>
  );
}

function Card({ children, style, onClick }) {
  return (
    <div
      onClick={onClick}
      style={{
        background: 'var(--bg-elevated)',
        border: '1px solid var(--border-subtle)',
        borderRadius: 'var(--radius-lg)',
        boxShadow: 'var(--shadow-xs)',
        padding: 16,
        ...(onClick ? { cursor: 'pointer' } : null),
        ...style,
      }}
    >
      {children}
    </div>
  );
}

function Toggle({ on, onClick }) {
  return (
    <button
      onClick={onClick}
      aria-pressed={on}
      style={{
        width: 38, height: 22,
        borderRadius: 99,
        background: on ? 'var(--brand-primary)' : 'var(--bg-muted)',
        position: 'relative',
        cursor: 'pointer',
        border: 'none', padding: 0,
        transition: 'background var(--dur-fast)',
      }}
    >
      <span style={{
        position: 'absolute',
        top: 2,
        [on ? 'insetInlineEnd' : 'insetInlineStart']: 2,
        width: 18, height: 18,
        borderRadius: '50%',
        background: '#fff',
        boxShadow: 'var(--shadow-xs)',
        transition: 'all var(--dur-fast)',
      }} />
    </button>
  );
}

window.UI = { Button, Field, Input, Badge, Card, Toggle, cls };
