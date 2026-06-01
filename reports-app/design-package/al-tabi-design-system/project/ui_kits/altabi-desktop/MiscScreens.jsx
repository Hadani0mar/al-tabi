/* global React, Icon, UI */
// Notifications log — تنبيهات tab. Simple list.

const ALERTS = [
  { id: 'a1', tone: 'warning', title: '3 أصناف قاربت على النفاد',     when: 'قبل 12 د',  body: 'باراسيتامول, سيتريزين, ميتفورمين' },
  { id: 'a2', tone: 'success', title: 'تقرير المبيعات اليومي أُرسل',   when: 'اليوم 08:00', body: 'إلى @ahmed على Telegram · 47 صفاً' },
  { id: 'a3', tone: 'info',    title: 'محادثة محفوظة',                  when: 'اليوم 07:42', body: '«ديون الموردين النشطة» أضيفت إلى المحفوظات' },
  { id: 'a4', tone: 'danger',  title: 'فشل تشغيل: مرتجعات الأسبوع',    when: 'الأمس 18:11', body: 'انقطاع شبكي مؤقت — تمت إعادة المحاولة ونجحت' },
  { id: 'a5', tone: 'success', title: 'تم تحديث الـ schema',           when: 'الأمس 12:00', body: 'تم اكتشاف جدول جديد: PurchaseReturns' },
  { id: 'a6', tone: 'warning', title: 'صنف قارب على انتهاء الصلاحية',  when: 'قبل يومين',   body: 'أزيثرومايسين 250mg — 47 وحدة تنتهي خلال 30 يوماً' },
];

function AlertsScreen() {
  return (
    <div style={{ width: '100%', height: '100%', overflow: 'auto', paddingBottom: 90 }}>
      <div style={{
        padding: '20px 28px 14px',
        borderBottom: '1px solid var(--border-subtle)',
        display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end',
      }}>
        <div>
          <h1 className="t-h1" style={{ margin: 0 }}>التنبيهات</h1>
          <p style={{ fontSize: 13.5, color: 'var(--fg-2)', marginTop: 4, marginBottom: 0 }}>
            سجل آخر <span className="t-numeric" style={{ fontWeight: 600 }}>{ALERTS.length}</span> تنبيه.
          </p>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <UI.Button variant="ghost" size="md">تعليم الكل كمقروء</UI.Button>
        </div>
      </div>

      <div style={{ padding: '20px 28px', display: 'flex', flexDirection: 'column', gap: 8 }}>
        {ALERTS.map((a) => (
          <UI.Card key={a.id} style={{
            padding: '12px 14px',
            display: 'grid',
            gridTemplateColumns: 'auto 1fr auto',
            gap: 12, alignItems: 'flex-start',
          }}>
            <div style={{
              width: 30, height: 30, borderRadius: 8,
              background: `var(--${a.tone}-soft)`,
              color:      `var(--${a.tone})`,
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              flexShrink: 0,
              marginTop: 1,
            }}>
              {a.tone === 'success' ? <Icon.Check size={16}/> :
               a.tone === 'warning' ? <Icon.Bell size={16}/> :
               a.tone === 'danger'  ? <Icon.X size={16}/> :
                                      <Icon.Info size={16}/>}
            </div>
            <div>
              <div style={{ fontSize: 13.5, fontWeight: 600 }}>{a.title}</div>
              <div style={{ fontSize: 12.5, color: 'var(--fg-2)', marginTop: 2 }}>{a.body}</div>
            </div>
            <div style={{ fontSize: 11.5, color: 'var(--fg-3)', whiteSpace: 'nowrap' }}>{a.when}</div>
          </UI.Card>
        ))}
      </div>
    </div>
  );
}

function SearchScreen() {
  const [query, setQuery] = React.useState('');
  return (
    <div style={{ width: '100%', height: '100%', overflow: 'auto', paddingBottom: 90 }}>
      <div style={{ padding: '20px 28px 14px', borderBottom: '1px solid var(--border-subtle)' }}>
        <h1 className="t-h1" style={{ margin: 0 }}>كتالوج التقارير</h1>
        <p style={{ fontSize: 13.5, color: 'var(--fg-2)', marginTop: 4, marginBottom: 14 }}>
          ابحث في التقارير الجاهزة من المكتبة المركزية.
        </p>
        <div style={{ position: 'relative', maxWidth: 540 }}>
          <Icon.Search size={16}/>
          <span style={{ position: 'absolute', insetInlineEnd: 14, top: 12, color: 'var(--fg-3)' }}><Icon.Search size={16}/></span>
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="مثال: مبيعات، ديون، مخزون…"
            style={{
              width: '100%',
              padding: '10px 42px 10px 14px',
              fontFamily: 'var(--font-ui)',
              fontSize: 14,
              border: '1px solid var(--border-default)',
              borderRadius: 'var(--radius-md)',
              background: 'var(--bg-elevated)',
              outline: 'none',
              boxSizing: 'border-box',
            }}
          />
        </div>
      </div>
      <div style={{ padding: '24px 28px' }}>
        <div style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--fg-3)', letterSpacing: '0.06em', textTransform: 'uppercase', marginBottom: 10 }}>
          المجموعات الشائعة
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 10 }}>
          {[
            { name: 'مبيعات', count: 24, tone: 'brand' },
            { name: 'مخزون', count: 18, tone: 'warning' },
            { name: 'مالية', count: 31, tone: 'accent' },
            { name: 'موردون', count: 12, tone: 'info' },
            { name: 'عملاء', count: 9, tone: 'success' },
            { name: 'مرتجعات', count: 6, tone: 'neutral' },
          ].map((g) => (
            <UI.Card key={g.name} style={{ padding: '14px 16px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div>
                <div style={{ fontSize: 14, fontWeight: 600 }}>{g.name}</div>
                <div style={{ fontSize: 11.5, color: 'var(--fg-3)', marginTop: 2 }}>
                  <span className="t-numeric">{g.count}</span> تقرير جاهز
                </div>
              </div>
              <UI.Badge tone={g.tone}>{g.count}</UI.Badge>
            </UI.Card>
          ))}
        </div>
      </div>
    </div>
  );
}

window.AlertsScreen = AlertsScreen;
window.SearchScreen = SearchScreen;
