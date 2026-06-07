import { useState } from 'react';
import { ToastProvider } from './lib/ToastContext';
import { useSessions } from './lib/useSessions';
import Navbar from './components/Navbar';
import Hero from './components/Hero';
import DownloadConsole from './components/DownloadConsole';
import SessionsPanel from './components/SessionsPanel';
import SettingsPanel from './components/SettingsPanel';

function Shell() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { sessions, connection, addLaunched, cancel, dismiss } = useSessions();

  return (
    <div className="min-h-screen bg-paper text-ink">
      <Navbar
        connection={connection}
        address={typeof window !== 'undefined' ? window.location.host : ''}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <main>
        <Hero />
        <DownloadConsole onLaunched={addLaunched} />
        <SessionsPanel sessions={sessions} onCancel={cancel} onDismiss={dismiss} />
      </main>
      <footer className="border-t border-line/70 px-6 py-8 text-center text-xs text-ink-faint">
        vdl runs entirely on your machine — nothing you download ever leaves it.
      </footer>
      <SettingsPanel open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}

export default function App() {
  return (
    <ToastProvider>
      <Shell />
    </ToastProvider>
  );
}
