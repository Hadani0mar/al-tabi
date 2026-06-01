/* global React, Icon, UI */
// AI Chat — the heart of al-tabi. الذكاء tab.

const SUGGESTIONS = [
  'كم مبيعات اليوم؟',
  'ديون الموردين الأعلى',
  'أصناف قاربت على النفاد',
  'مقارنة المبيعات هذا الشهر بالماضي',
];

const FIXED_REPLY = {
  query: "SELECT TOP 5 i.name_ar AS الصنف, SUM(s.qty) AS الكمية, SUM(s.total) AS الإجمالي\nFROM Sales s JOIN Items i ON i.id = s.item_id\nWHERE s.date = CAST(GETDATE() AS DATE)\nGROUP BY i.name_ar\nORDER BY الإجمالي DESC",
  rowCount: 5,
  totalSales: '42,180',
  deltaPercent: '+8.4',
  rows: [
    ['أموكسيسيلين 500mg',    '1,240', '38,420.00', 'pos', '+12.4%'],
    ['باراسيتامول 500mg',    '890',   '14,560.00', 'neg', '−3.2%'],
    ['أوميبرازول 20mg',      '402',   '9,840.00',  'pos', '+1.8%'],
    ['سيتريزين 10mg',        '316',   '5,290.00',  'pos', '+4.0%'],
    ['ميتفورمين 850mg',      '290',   '4,820.00',  'neg', '−0.6%'],
  ],
};

function ToolStatusPill({ running, query }) {
  return (
    <div style={{
      display: 'inline-flex',
      alignItems: 'center', gap: 8,
      padding: '6px 12px',
      borderRadius: 'var(--radius-pill)',
      background: 'var(--bg-subtle)',
      border: '1px solid var(--border-subtle)',
      fontSize: 11.5,
      fontFamily: 'var(--font-mono)',
      color: 'var(--fg-2)',
      maxWidth: '85%',
      alignSelf: 'flex-end',
    }}>
      <span style={{
        width: 6, height: 6, borderRadius: '50%',
        background: running ? 'var(--brand-accent)' : 'var(--success)',
        animation: running ? 'pulse 1.2s infinite' : 'none',
      }}/>
      {running ? 'جارٍ تنفيذ الاستعلام…' : 'تم التنفيذ ' + Math.round(420 + Math.random()*200) + 'ms'}
      <span style={{
        maxWidth: 360,
        overflow: 'hidden',
        textOverflow: 'ellipsis',
        whiteSpace: 'nowrap',
        color: 'var(--fg-3)',
        marginInlineStart: 6,
      }}>{query}</span>
    </div>
  );
}

function ResultTable() {
  return (
    <div style={{
      marginTop: 10,
      background: 'var(--bg-elevated)',
      border: '1px solid var(--border-subtle)',
      borderRadius: 'var(--radius-md)',
      overflow: 'hidden',
    }}>
      <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
        <thead>
          <tr style={{ background: 'var(--bg-subtle)' }}>
            {['#','الصنف','الكمية','الإجمالي (د.ل)','الفرق'].map((h, i) => (
              <th key={i} style={{
                padding: '8px 10px',
                fontSize: 11, fontWeight: 600,
                color: 'var(--fg-2)',
                letterSpacing: '0.05em',
                textTransform: 'uppercase',
                textAlign: i >= 2 ? 'end' : 'start',
                borderBottom: '1px solid var(--border-default)',
              }}>{h}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {FIXED_REPLY.rows.map((row, i) => (
            <tr key={i} style={{ borderBottom: i < FIXED_REPLY.rows.length - 1 ? '1px solid var(--border-subtle)' : 'none' }}>
              <td style={{ padding: '8px 10px', fontFamily: 'var(--font-numeric)', fontVariantNumeric: 'tabular-nums', color: 'var(--fg-3)' }}>{i+1}</td>
              <td style={{ padding: '8px 10px' }}>{row[0]}</td>
              <td style={{ padding: '8px 10px', fontFamily: 'var(--font-numeric)', fontVariantNumeric: 'tabular-nums', textAlign: 'end' }}>{row[1]}</td>
              <td style={{ padding: '8px 10px', fontFamily: 'var(--font-numeric)', fontVariantNumeric: 'tabular-nums', textAlign: 'end', fontWeight: 600 }}>{row[2]}</td>
              <td style={{ padding: '8px 10px', fontFamily: 'var(--font-numeric)', fontVariantNumeric: 'tabular-nums', textAlign: 'end', fontWeight: 600, color: row[3] === 'pos' ? 'var(--currency-pos)' : 'var(--currency-neg)' }}>{row[4]}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function AIChatScreen() {
  const [messages, setMessages] = React.useState([]);
  const [input, setInput] = React.useState('');
  const [running, setRunning] = React.useState(false);
  const streamRef = React.useRef(null);

  React.useEffect(() => {
    if (streamRef.current) streamRef.current.scrollTop = streamRef.current.scrollHeight;
  }, [messages, running]);

  const ask = (q) => {
    if (!q.trim()) return;
    setMessages((m) => [...m, { kind: 'user', text: q }]);
    setInput('');
    setRunning(true);
    setTimeout(() => {
      setMessages((m) => [...m, { kind: 'tool', query: FIXED_REPLY.query }]);
    }, 400);
    setTimeout(() => {
      setRunning(false);
      setMessages((m) => [...m, { kind: 'ai', table: true }]);
    }, 1700);
  };

  const empty = messages.length === 0;

  return (
    <div style={{
      width: '100%', height: '100%',
      display: 'flex', flexDirection: 'column',
      paddingBottom: 90,
    }}>
      {/* Header strip */}
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        padding: '12px 24px',
        borderBottom: '1px solid var(--border-subtle)',
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <div style={{
            width: 30, height: 30, borderRadius: 8,
            background: 'var(--brand-accent-soft)',
            color: 'var(--brand-accent)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}><Icon.Sparkles size={17}/></div>
          <div>
            <div style={{ fontSize: 14, fontWeight: 600 }}>الذكاء</div>
            <div style={{ fontSize: 11.5, color: 'var(--fg-3)' }}>محلل خبير على Marketing2026</div>
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <UI.Button variant="secondary" size="sm" icon={<Icon.Plus size={14}/>}>محادثة جديدة</UI.Button>
        </div>
      </div>

      {/* Stream */}
      <div ref={streamRef} style={{
        flex: 1, overflow: 'auto',
        padding: '20px 24px',
        display: 'flex', flexDirection: 'column', gap: 10,
      }}>
        {empty && (
          <div style={{
            margin: 'auto 0',
            textAlign: 'center',
            display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 14,
            padding: '40px 24px',
          }}>
            <div style={{
              width: 56, height: 56, borderRadius: 14,
              background: 'var(--brand-accent-soft)',
              color: 'var(--brand-accent)',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}><Icon.Sparkles size={28}/></div>
            <div className="t-h2" style={{ margin: 0 }}>اسأل عن بياناتك بالعربية</div>
            <div style={{ fontSize: 13.5, color: 'var(--fg-2)', maxWidth: 460, lineHeight: 1.55 }}>
              اكتب سؤالاً أو اختر اقتراحاً للبدء. سيتم تنفيذ استعلام آمن (قراءة فقط) على Marketing2026.
            </div>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, justifyContent: 'center', maxWidth: 600, marginTop: 8 }}>
              {SUGGESTIONS.map((s) => (
                <button key={s} onClick={() => ask(s)}
                  style={{
                    fontFamily: 'var(--font-ui)',
                    fontSize: 13,
                    padding: '8px 14px',
                    border: '1px solid var(--border-default)',
                    background: 'var(--bg-surface)',
                    borderRadius: 'var(--radius-pill)',
                    color: 'var(--fg-1)',
                    cursor: 'pointer',
                    transition: 'all var(--dur-fast)',
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.borderColor = 'var(--brand-primary)'; e.currentTarget.style.color = 'var(--brand-primary)'; }}
                  onMouseLeave={(e) => { e.currentTarget.style.borderColor = 'var(--border-default)'; e.currentTarget.style.color = 'var(--fg-1)'; }}
                >
                  {s}
                </button>
              ))}
            </div>
          </div>
        )}

        {messages.map((m, i) => {
          if (m.kind === 'user') return (
            <div key={i} style={{
              alignSelf: 'flex-start',
              maxWidth: '76%',
              padding: '10px 14px',
              background: 'var(--user-bubble-bg)',
              color: 'var(--user-bubble-fg)',
              borderRadius: 14,
              borderBottomRightRadius: 4,
              fontSize: 14, lineHeight: 1.5,
            }}>{m.text}</div>
          );
          if (m.kind === 'tool') return <ToolStatusPill key={i} running={false} query={m.query.split('\n')[0]} />;
          if (m.kind === 'ai') return (
            <div key={i} style={{
              alignSelf: 'flex-end',
              maxWidth: '88%',
              padding: '12px 16px',
              background: 'var(--ai-bubble-bg)',
              color: 'var(--ai-bubble-fg)',
              borderRadius: 14,
              borderBottomLeftRadius: 4,
              border: '1px solid var(--ai-bubble-border)',
              fontSize: 14, lineHeight: 1.6,
            }}>
              مبيعات اليوم بلغت <b className="t-numeric" style={{ color: 'var(--fg-1)' }}>{FIXED_REPLY.totalSales}</b> د.ل (<span style={{ color: 'var(--currency-pos)', fontWeight: 600 }}>{FIXED_REPLY.deltaPercent}%</span> عن أمس). أعلى ٥ أصناف:
              <ResultTable />
              <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
                <UI.Button variant="secondary" size="sm" icon={<Icon.Download size={14}/>}>تصدير Excel</UI.Button>
                <UI.Button variant="secondary" size="sm" icon={<Icon.File size={14}/>}>PDF</UI.Button>
                <UI.Button variant="ghost" size="sm" icon={<Icon.Save size={14}/>}>حفظ كمحفوظة</UI.Button>
                <UI.Button variant="ghost" size="sm" icon={<Icon.Calendar size={14}/>}>جدولة</UI.Button>
              </div>
            </div>
          );
          return null;
        })}

        {running && <ToolStatusPill running query={FIXED_REPLY.query.split('\n')[0]} />}
      </div>

      {/* Composer */}
      <div style={{
        padding: '14px 24px 16px',
        borderTop: '1px solid var(--border-subtle)',
        background: 'var(--bg-surface)',
      }}>
        <form
          onSubmit={(e) => { e.preventDefault(); ask(input); }}
          style={{
            display: 'flex', gap: 8, alignItems: 'flex-end',
            background: 'var(--bg-elevated)',
            border: '1px solid var(--border-default)',
            borderRadius: 'var(--radius-lg)',
            padding: 6,
          }}>
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="اكتب سؤالك… مثال: مبيعات الأسبوع لكل صنف"
            style={{
              flex: 1,
              padding: '10px 12px',
              border: 'none', outline: 'none',
              background: 'transparent',
              fontFamily: 'var(--font-ui)',
              fontSize: 14, color: 'var(--fg-1)',
            }}
          />
          <UI.Button variant="primary" size="md" icon={<Icon.Send size={14}/>} type="submit">إرسال</UI.Button>
        </form>
        <div style={{ fontSize: 11, color: 'var(--fg-3)', marginTop: 8, paddingInlineStart: 4 }}>
          الاتصال قراءة فقط — لن يتم تعديل بيانات Marketing2026.
        </div>
      </div>
    </div>
  );
}

window.AIChatScreen = AIChatScreen;
