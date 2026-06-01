/* global React, Icon, UI */
// Reports / Scheduler — التقارير tab.

const SCHEDULES_INIT = [
  {
    id: 's1',
    name: 'تقرير المبيعات اليومي',
    desc: 'إجمالي المبيعات وأعلى 10 أصناف',
    cadence: 'يومي · 08:00',
    target: 'Telegram',
    on: true,
    lastRun: 'اليوم 08:00',
    runs: 142,
  },
  {
    id: 's2',
    name: 'ديون الموردين',
    desc: 'كشف ديون الموردين النشطين',
    cadence: 'أسبوعي · الأحد 09:00',
    target: 'PDF + Email',
    on: true,
    lastRun: 'الأحد الماضي',
    runs: 18,
  },
  {
    id: 's3',
    name: 'تنبيه أصناف قاربت على النفاد',
    desc: 'الأصناف بكمية ≤ 50',
    cadence: 'كل ساعتين',
    target: 'Telegram',
    on: false,
    lastRun: 'قبل 3 أيام',
    runs: 87,
  },
  {
    id: 's4',
    name: 'تقرير المخزون الشهري',
    desc: 'الكمية والقيمة لكل مجموعة',
    cadence: 'شهري · 1 من الشهر',
    target: 'Excel',
    on: true,
    lastRun: 'الشهر الماضي',
    runs: 6,
  },
];

function ScheduleCard({ s, onToggle, onMenu }) {
  return (
    <UI.Card style={{ padding: '14px 16px' }}>
      <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr auto', gap: 14, alignItems: 'center' }}>
        <div style={{
          width: 38, height: 38, borderRadius: 10,
          background: s.on ? 'var(--brand-primary-soft)' : 'var(--bg-muted)',
          color:      s.on ? 'var(--brand-primary-ink)' : 'var(--fg-3)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
        }}>
          <Icon.Calendar size={18}/>
        </div>
        <div>
          <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 2 }}>{s.name}</div>
          <div style={{ fontSize: 12, color: 'var(--fg-3)' }}>{s.desc}</div>
          <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginTop: 8 }}>
            <UI.Badge tone={s.on ? 'success' : 'neutral'} dot>{s.cadence}</UI.Badge>
            <span style={{ fontSize: 11.5, color: 'var(--fg-3)' }}>· {s.target}</span>
            <span style={{ fontSize: 11.5, color: 'var(--fg-3)' }}>· آخر تشغيل {s.lastRun}</span>
            <span style={{ fontSize: 11.5, color: 'var(--fg-3)' }}>· <span className="t-numeric">{s.runs}</span> تشغيلة</span>
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <UI.Button variant="ghost" size="sm" icon={<Icon.Play size={14}/>} onClick={onMenu}>تشغيل الآن</UI.Button>
          <UI.Toggle on={s.on} onClick={onToggle}/>
        </div>
      </div>
    </UI.Card>
  );
}

function ReportsScreen() {
  const [schedules, setSchedules] = React.useState(SCHEDULES_INIT);
  const toggle = (id) => setSchedules((xs) => xs.map((s) => s.id === id ? { ...s, on: !s.on } : s));
  const active = schedules.filter((s) => s.on).length;

  return (
    <div style={{ width: '100%', height: '100%', overflow: 'auto', paddingBottom: 90 }}>
      <div style={{
        padding: '20px 28px 14px',
        borderBottom: '1px solid var(--border-subtle)',
        display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end',
      }}>
        <div>
          <h1 className="t-h1" style={{ margin: 0 }}>التقارير المجدولة</h1>
          <p style={{ fontSize: 13.5, color: 'var(--fg-2)', marginTop: 4, marginBottom: 0 }}>
            <span className="t-numeric" style={{ fontWeight: 600 }}>{active}</span> نشطة من
            <span className="t-numeric" style={{ fontWeight: 600 }}> {schedules.length}</span> — تشتغل تلقائياً وتُرسَل إلى Telegram والبريد.
          </p>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <UI.Button variant="secondary" size="md" icon={<Icon.Filter size={14}/>}>تصفية</UI.Button>
          <UI.Button variant="primary" size="md" icon={<Icon.Plus size={14}/>}>جدولة جديدة</UI.Button>
        </div>
      </div>

      {/* Stat strip */}
      <div style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(4, 1fr)',
        gap: 14,
        padding: '20px 28px 0',
      }}>
        {[
          { label: 'نشطة',         value: active, tone: 'success' },
          { label: 'تشغيل اليوم', value: 7, tone: 'brand' },
          { label: 'فشل هذا الأسبوع', value: 0, tone: 'neutral' },
          { label: 'متوسط المدة', value: '0.6 ث', tone: 'accent' },
        ].map((k) => (
          <UI.Card key={k.label} style={{ padding: '12px 16px' }}>
            <div style={{ fontSize: 11.5, color: 'var(--fg-3)', fontWeight: 600, letterSpacing: '0.04em', textTransform: 'uppercase' }}>
              {k.label}
            </div>
            <div className="t-numeric" style={{ fontSize: 24, fontWeight: 700, marginTop: 4 }}>{k.value}</div>
          </UI.Card>
        ))}
      </div>

      {/* List */}
      <div style={{ padding: '20px 28px', display: 'flex', flexDirection: 'column', gap: 10 }}>
        <div style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--fg-3)', letterSpacing: '0.06em', textTransform: 'uppercase', marginBottom: 4 }}>
          القائمة الكاملة
        </div>
        {schedules.map((s) => (
          <ScheduleCard key={s.id} s={s} onToggle={() => toggle(s.id)} onMenu={() => {}}/>
        ))}
      </div>
    </div>
  );
}

window.ReportsScreen = ReportsScreen;
