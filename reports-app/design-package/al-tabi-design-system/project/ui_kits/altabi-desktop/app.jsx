/* global React, ReactDOM, AppShell,
   LoginScreen, AIChatScreen, ReportsScreen, SavedScreen, SettingsScreen,
   AlertsScreen, SearchScreen */

function App() {
  const [loggedIn, setLoggedIn] = React.useState(false);
  const [tab, setTab] = React.useState('ai');
  const [themeMode, setThemeMode] = React.useState('light');

  // Persist theme
  React.useEffect(() => {
    const saved = localStorage.getItem('altabi:theme');
    if (saved) setThemeMode(saved);
  }, []);
  React.useEffect(() => {
    localStorage.setItem('altabi:theme', themeMode);
    document.documentElement.setAttribute('data-theme', themeMode === 'dark' ? 'mihbar-dark' : 'mihbar');
  }, [themeMode]);

  const screen = (() => {
    switch (tab) {
      case 'reports':  return <ReportsScreen/>;
      case 'search':   return <SearchScreen/>;
      case 'alerts':   return <AlertsScreen/>;
      case 'ai':       return <AIChatScreen/>;
      case 'saved':    return <SavedScreen/>;
      case 'settings': return <SettingsScreen themeMode={themeMode} onThemeMode={setThemeMode}/>;
      default:         return <AIChatScreen/>;
    }
  })();

  if (!loggedIn) {
    return (
      <AppShell businessName="صيدلية الشفاء" connected={false} showNav={false}>
        <LoginScreen onConnect={() => setLoggedIn(true)} />
      </AppShell>
    );
  }

  return (
    <AppShell businessName="صيدلية الشفاء" connected={true} active={tab} onChangeTab={setTab}>
      {screen}
    </AppShell>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
