/* global React, Icon, UI */
// Saved queries — المحفوظات tab.

const SAVED = [
  { id: 'q1', name: 'مبيعات اليوم',                runs: 412, lastRun: 'قبل دقيقة', category: 'مبيعات' },
  { id: 'q2', name: 'أعلى 10 أصناف هذا الشهر',     runs: 28,  lastRun: 'اليوم', category: 'مبيعات' },
  { id: 'q3', name: 'ديون الموردين النشطة',         runs: 56,  lastRun: 'الأمس', category: 'مالية' },
  { id: 'q4', name: 'مرتجعات الأسبوع',              runs: 14,  lastRun: 'قبل ساعة', category: 'مبيعات' },
  { id: 'q5', name: 'أصناف بكمية ≤ 50',             runs: 198, lastRun: 'قبل 12 د', category: 'مخزون' },
  { id: 'q6', name: 'أصناف قاربت على انتهاء الصلاحية', runs: 9,   lastRun: 'هذا الأسبوع', category: 'مخزون' },
  { id: 'q7', name: 'كشف حساب مورد',                runs: 33,  lastRun: 'الأمس', category: 'مالية' },
  { id: 'q8', name: 'المبيعات حسب البائع',         runs: 21,  lastRun: 'الأمس', category: 'مبيعات' },
];

function SavedScreen() {
  const [filter, setFilter] = React.useState('الكل');
  const cats = ['الكل', 'مبيعات', 'مخزون', 'مالية'];
  const filtered = SAVED.filter((q) => filter === 'الكل' || q.category === filter);

  const toneFor = (c) => c === 'مبيعات' ? 'brand' : c === 'مخزون' ? 'warning' : c === 'مالية' ? 'accent' : 'neutral';

  return (
    <div style={{ width: '100%', height: '100%', overflow: 'auto', paddingBottom: 90 }}>
      <div style={{
        padding: '20px 28px 14px',
        borderBottom: '1px solid var(--border-subtle)',
        display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end',
      }}>
        <div>
          <h1 className="t-h1" style={{ margin: 0 }}>المحفوظات</h1>
          <p style={{ fontSize: 13.5, color: 'var(--fg-2)', marginTop: 4, marginBottom: 0 }}>
            استعلامات مُختبَرة — شغّلها بنقرة دون الحاجة للذكاء.
          </p>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <UI.Button variant="secondary" size="md" icon={<Icon.Search size={14}/>}>بحث</UI.Button>
          <UI.Button variant="primary" size="md" icon={<Icon.Plus size={14}/>}>محفوظة جديدة</UI.Button>
        </div>
      </div>

      <div style={{ padding: '18px 28px 0', display: 'flex', gap: 6 }}>
        {cats.map((c) => (
          <button key={c}
            onClick={() => setFilter(c)}
            style={{
              fontFamily: 'var(--font-ui)',
              fontSize: 12.5, fontWeight: 600,
              padding: '6px 14px',
              borderRadius: 'var(--radius-pill)',
              border: '1px solid ' + (filter === c ? 'var(--brand-primary)' : 'var(--border-default)'),
              background: filter === c ? 'var(--brand-primary-soft)' : 'var(--bg-surface)',
              color: filter === c ? 'var(--brand-primary-ink)' : 'var(--fg-2)',
              cursor: 'pointer',
              transition: 'all var(--dur-fast)',
            }}>{c}</button>
        ))}
      </div>

      <div style={{
        padding: '14px 28px 28px',
        display: 'grid',
        gridTemplateColumns: 'repeat(2, 1fr)',
        gap: 10,
      }}>
        {filtered.map((q) => (
          <UI.Card key={q.id} style={{ padding: '14px 16px' }}>
            <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr auto', gap: 12, alignItems: 'center' }}>
              <div style={{
                width: 36, height: 36, borderRadius: 9,
                background: 'var(--bg-subtle)',
                color: 'var(--fg-1)',
                display: 'flex', alignItems: 'center', justifyContent: 'center',
              }}><Icon.Bookmark size={17}/></div>
              <div>
                <div style={{ fontSize: 14, fontWeight: 600 }}>{q.name}</div>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginTop: 4 }}>
                  <UI.Badge tone={toneFor(q.category)}>{q.category}</UI.Badge>
                  <span style={{ fontSize: 11.5, color: 'var(--fg-3)' }}>
                    <span className="t-numeric">{q.runs}</span> تشغيلة · {q.lastRun}
                  </span>
                </div>
              </div>
              <UI.Button variant="ghost" size="sm" icon={<Icon.Play size={14}/>}>شغّل</UI.Button>
            </div>
          </UI.Card>
        ))}
      </div>
    </div>
  );
}

window.SavedScreen = SavedScreen;
