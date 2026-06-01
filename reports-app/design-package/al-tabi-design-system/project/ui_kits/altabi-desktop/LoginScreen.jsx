/* global React, Icon, UI */
// Login — SQL Server connection.

function LoginScreen({ onConnect }) {
  const [state, setState] = React.useState({
    server: 'SQLSRV-01\\MARKET',
    database: 'Marketing2026',
    user: 'sa',
    password: '••••••••',
    saved: true,
  });
  const [showPwd, setShowPwd] = React.useState(false);
  const [connecting, setConnecting] = React.useState(false);

  const set = (k) => (v) => setState({ ...state, [k]: v });

  const onSubmit = (e) => {
    e.preventDefault();
    setConnecting(true);
    setTimeout(() => {
      setConnecting(false);
      onConnect(state);
    }, 1200);
  };

  return (
    <div style={{
      width: '100%', height: '100%',
      display: 'grid',
      gridTemplateColumns: '1fr 1fr',
      background: 'var(--bg-canvas)',
    }}>
      {/* Left — brand panel */}
      <div style={{
        background: 'linear-gradient(160deg, #0F6E70 0%, #0A5759 100%)',
        color: 'var(--fg-on-brand)',
        padding: 56,
        display: 'flex',
        flexDirection: 'column',
        justifyContent: 'space-between',
        position: 'relative',
        overflow: 'hidden',
      }}>
        {/* subtle paper texture: copper spark in bottom corner */}
        <div style={{
          position: 'absolute',
          bottom: -80, insetInlineEnd: -80,
          width: 320, height: 320,
          borderRadius: '50%',
          background: 'radial-gradient(circle, rgba(184,106,44,0.30), transparent 65%)',
          pointerEvents: 'none',
        }}/>

        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <img src="../../assets/logo-mark.svg" alt="" width={44} height={44} style={{ borderRadius: 10 }}/>
          <div style={{ fontFamily: 'var(--font-display)', fontSize: 22, fontWeight: 700 }}>
            التابي
            <div style={{ fontFamily: 'var(--font-ui)', fontSize: 11, fontWeight: 500, opacity: 0.7, letterSpacing: '0.04em', marginTop: 2 }}>
              AL-TABI · REPORTS
            </div>
          </div>
        </div>

        <div style={{ position: 'relative' }}>
          <div style={{
            fontFamily: 'var(--font-display)',
            fontSize: 32, fontWeight: 700,
            lineHeight: 1.3,
            marginBottom: 16,
            letterSpacing: '-0.01em',
          }}>
            ذكاء التقارير<br/>فوق Marketing2026
          </div>
          <p style={{
            fontSize: 14.5, fontWeight: 400, opacity: 0.82,
            lineHeight: 1.65, maxWidth: 360,
            margin: 0,
          }}>
            اسأل بالعربية. احصل على الجواب فوراً — جدول، PDF، أو Excel.
            بيانات على جهازك، اتصال قراءة فقط.
          </p>

          <div style={{ display: 'flex', gap: 18, marginTop: 32 }}>
            {[
              { icon: <Icon.Check size={16}/>, text: 'قراءة فقط' },
              { icon: <Icon.Database size={16}/>, text: 'SQL Server' },
              { icon: <Icon.Telegram size={16}/>, text: 'Telegram' },
            ].map((f) => (
              <div key={f.text} style={{
                display: 'flex', alignItems: 'center', gap: 6,
                fontSize: 12, opacity: 0.85,
                whiteSpace: 'nowrap',
              }}>{f.icon}{f.text}</div>
            ))}
          </div>
        </div>

        <div style={{ fontSize: 11, opacity: 0.55, fontFamily: 'var(--font-mono)' }}>
          v0.1.4 · build 2026.05
        </div>
      </div>

      {/* Right — form */}
      <div style={{
        padding: 64,
        display: 'flex',
        flexDirection: 'column',
        justifyContent: 'center',
      }}>
        <div style={{ marginBottom: 28 }}>
          <div style={{
            fontSize: 11, color: 'var(--brand-primary)',
            fontWeight: 600, letterSpacing: '0.08em',
            textTransform: 'uppercase',
            marginBottom: 8,
            fontFamily: 'var(--font-mono)',
          }}>الخطوة الأولى</div>
          <h1 className="t-h1" style={{ margin: 0 }}>اتصل بقاعدة البيانات</h1>
          <p style={{ fontSize: 13.5, color: 'var(--fg-2)', marginTop: 8, lineHeight: 1.6 }}>
            أدخل بيانات SQL Server الخاصة بـ Marketing2026.
            يتم حفظ بيانات الاتصال بأمان على جهازك فقط.
          </p>
        </div>

        <form onSubmit={onSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 14, maxWidth: 420 }}>
          <UI.Field label="الخادم">
            <UI.Input value={state.server} onChange={set('server')} />
          </UI.Field>
          <UI.Field label="قاعدة البيانات">
            <UI.Input value={state.database} onChange={set('database')} />
          </UI.Field>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
            <UI.Field label="المستخدم">
              <UI.Input value={state.user} onChange={set('user')} />
            </UI.Field>
            <UI.Field label="كلمة المرور">
              <div style={{ position: 'relative' }}>
                <UI.Input
                  type={showPwd ? 'text' : 'password'}
                  value={state.password}
                  onChange={set('password')}
                />
                <button
                  type="button"
                  onClick={() => setShowPwd(!showPwd)}
                  style={{
                    position: 'absolute', insetInlineStart: 8, top: 8,
                    width: 24, height: 24,
                    background: 'transparent', border: 'none',
                    color: 'var(--fg-3)', cursor: 'pointer',
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                  }}
                  aria-label={showPwd ? 'إخفاء' : 'إظهار'}
                >
                  {showPwd ? <Icon.EyeOff size={16}/> : <Icon.Eye size={16}/>}
                </button>
              </div>
            </UI.Field>
          </div>

          <label style={{ display: 'inline-flex', alignItems: 'center', gap: 8, marginTop: 6, fontSize: 13, color: 'var(--fg-2)', whiteSpace: 'nowrap', alignSelf: 'flex-start' }}>
            <input
              type="checkbox"
              checked={state.saved}
              onChange={(e) => set('saved')(e.target.checked)}
              style={{ accentColor: 'var(--brand-primary)', width: 16, height: 16, flexShrink: 0 }}
            />
            تذكّر الاتصال
          </label>

          <div style={{ marginTop: 14, alignSelf: 'flex-start' }}>
            <UI.Button
              type="submit"
              variant="primary"
              size="lg"
              disabled={connecting}
              icon={connecting ? null : <Icon.Database size={16}/>}
              style={{ whiteSpace: 'nowrap' }}
            >
              {connecting ? 'جارٍ الاتصال…' : 'اتصل بقاعدة البيانات'}
            </UI.Button>
          </div>
        </form>

        <div style={{
          marginTop: 28,
          paddingTop: 16,
          borderTop: '1px solid var(--border-subtle)',
          fontSize: 12, color: 'var(--fg-3)',
        }}>
          مشاكل في الاتصال؟ <a href="#help" style={{ color: 'var(--fg-link)' }}>راجع دليل إعداد SQL Server</a>
        </div>
      </div>
    </div>
  );
}

window.LoginScreen = LoginScreen;
