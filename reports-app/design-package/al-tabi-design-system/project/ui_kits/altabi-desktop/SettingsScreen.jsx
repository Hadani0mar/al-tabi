/* global React, Icon, UI */
// Settings — الإعدادات tab.

function SettingsRow({ icon, title, subtitle, children }) {
  return (
    <div style={{
      display: 'grid',
      gridTemplateColumns: 'auto 1fr auto',
      gap: 14, alignItems: 'center',
      padding: '14px 0',
      borderBottom: '1px solid var(--border-subtle)',
    }}>
      <div style={{
        width: 34, height: 34, borderRadius: 8,
        background: 'var(--bg-subtle)',
        color: 'var(--fg-1)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
      }}>{icon}</div>
      <div>
        <div style={{ fontSize: 13.5, fontWeight: 600 }}>{title}</div>
        {subtitle && <div style={{ fontSize: 12, color: 'var(--fg-3)', marginTop: 2 }}>{subtitle}</div>}
      </div>
      <div>{children}</div>
    </div>
  );
}

function SettingsSection({ title, children }) {
  return (
    <UI.Card style={{ padding: '4px 16px 14px', marginBottom: 14 }}>
      <div style={{
        fontSize: 11, fontWeight: 700,
        color: 'var(--brand-primary)',
        letterSpacing: '0.06em', textTransform: 'uppercase',
        padding: '12px 0 6px',
      }}>{title}</div>
      {children}
    </UI.Card>
  );
}

function SettingsScreen({ themeMode, onThemeMode }) {
  return (
    <div style={{ width: '100%', height: '100%', overflow: 'auto', paddingBottom: 90 }}>
      <div style={{
        padding: '20px 28px 14px',
        borderBottom: '1px solid var(--border-subtle)',
      }}>
        <h1 className="t-h1" style={{ margin: 0 }}>الإعدادات</h1>
        <p style={{ fontSize: 13.5, color: 'var(--fg-2)', marginTop: 4, marginBottom: 0 }}>
          إدارة الاتصالات، الإشعارات، والمظهر.
        </p>
      </div>

      <div style={{ padding: '20px 28px', maxWidth: 820 }}>

        <SettingsSection title="قاعدة البيانات">
          <SettingsRow
            icon={<Icon.Database size={17}/>}
            title="SQLSRV-01\MARKET · Marketing2026"
            subtitle="قراءة فقط · متصل منذ ساعتين"
          >
            <UI.Badge tone="success" dot>متصل</UI.Badge>
          </SettingsRow>
          <SettingsRow
            icon={<Icon.Bot size={17}/>}
            title="ذاكرة الـ schema"
            subtitle="34 جدولاً مفهرساً — يُحدَّث تلقائياً"
          >
            <UI.Button variant="secondary" size="sm">إعادة الفهرسة</UI.Button>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="الإشعارات و Telegram">
          <SettingsRow
            icon={<Icon.Telegram size={17}/>}
            title="بوت Telegram"
            subtitle="@altabi_reports_bot · 3 محادثات نشطة"
          >
            <UI.Badge tone="success" dot>متصل</UI.Badge>
          </SettingsRow>
          <SettingsRow
            icon={<Icon.Bell size={17}/>}
            title="تنبيهات سطح المكتب"
            subtitle="عرض إشعار عند اكتمال أي تقرير مجدول"
          >
            <UI.Toggle on={true} onClick={() => {}}/>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="المظهر">
          <SettingsRow
            icon={themeMode === 'dark' ? <Icon.Moon size={17}/> : <Icon.Sun size={17}/>}
            title="الوضع"
            subtitle={themeMode === 'dark' ? 'وضع ليلي — Lamplit Ledger' : 'وضع نهاري — Daylight Ledger'}
          >
            <div style={{ display: 'inline-flex', gap: 4, padding: 4, background: 'var(--bg-subtle)', borderRadius: 'var(--radius-pill)' }}>
              {[
                { id: 'light', label: 'نهاري', icon: <Icon.Sun size={13}/> },
                { id: 'dark',  label: 'ليلي',  icon: <Icon.Moon size={13}/> },
              ].map((m) => (
                <button key={m.id} onClick={() => onThemeMode(m.id)}
                  style={{
                    display: 'inline-flex', alignItems: 'center', gap: 6,
                    padding: '6px 12px',
                    borderRadius: 'var(--radius-pill)',
                    border: 'none', cursor: 'pointer',
                    fontSize: 12.5, fontWeight: 600,
                    fontFamily: 'var(--font-ui)',
                    background: themeMode === m.id ? 'var(--bg-elevated)' : 'transparent',
                    color: themeMode === m.id ? 'var(--fg-1)' : 'var(--fg-3)',
                    boxShadow: themeMode === m.id ? 'var(--shadow-sm)' : 'none',
                  }}>
                  {m.icon}{m.label}
                </button>
              ))}
            </div>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="بيانات النشاط التجاري">
          <SettingsRow
            icon={<Icon.File size={17}/>}
            title="صيدلية الشفاء — توزيع الأدوية"
            subtitle="طرابلس · رقم السجل 12834"
          >
            <UI.Button variant="ghost" size="sm" icon={<Icon.Edit size={14}/>}>تعديل</UI.Button>
          </SettingsRow>
          <SettingsRow
            icon={<Icon.Info size={17}/>}
            title="العملة الافتراضية"
            subtitle="دينار ليبي · د.ل · رقمان بعد العلامة"
          >
            <UI.Badge tone="neutral">د.ل</UI.Badge>
          </SettingsRow>
        </SettingsSection>

        <SettingsSection title="حول">
          <SettingsRow
            icon={<Icon.Info size={17}/>}
            title="al-tabi v0.1.4"
            subtitle="آخر تحقق من التحديثات: اليوم 09:14"
          >
            <UI.Button variant="ghost" size="sm">تحقق من التحديثات</UI.Button>
          </SettingsRow>
        </SettingsSection>

      </div>
    </div>
  );
}

window.SettingsScreen = SettingsScreen;
