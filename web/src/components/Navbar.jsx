const CONNECTION_COPY = {
  live: { label: 'Connected', dot: 'bg-teal' },
  connecting: { label: 'Connecting…', dot: 'bg-coral' },
  offline: { label: 'Reconnecting…', dot: 'bg-rose' },
};

export default function Navbar({ connection, address, onOpenSettings }) {
  const status = CONNECTION_COPY[connection] || CONNECTION_COPY.connecting;

  return (
    <header className="sticky top-0 z-40 border-b border-line/80 bg-paper/85 backdrop-blur-md">
      <div className="mx-auto flex max-w-6xl items-center justify-between gap-4 px-6 py-4">
        <a href="#top" className="flex items-baseline gap-2.5">
          <span className="font-display text-3xl italic text-coral">vdl</span>
          <span className="hidden text-xs uppercase tracking-[0.2em] text-ink-faint sm:inline">
            media downloader
          </span>
        </a>

        <nav className="hidden items-center gap-7 text-sm font-medium text-ink-soft md:flex">
          <a href="#download" className="transition hover:text-ink">Download</a>
          <a href="#sessions" className="transition hover:text-ink">Sessions</a>
          <a
            href="https://github.com/"
            target="_blank"
            rel="noreferrer"
            className="transition hover:text-ink"
          >
            Source
          </a>
        </nav>

        <div className="flex items-center gap-3">
          <span className="hidden items-center gap-2 rounded-full border border-line bg-card px-3 py-1.5 text-xs text-ink-soft sm:inline-flex">
            <span className={`h-2 w-2 rounded-full ${status.dot}`} />
            {status.label}
            {address && <span className="text-ink-faint">· {address}</span>}
          </span>
          <button
            type="button"
            onClick={onOpenSettings}
            className="rounded-full border border-line bg-card px-4 py-2 text-sm font-medium text-ink-soft transition hover:border-ink/30 hover:text-ink"
          >
            Settings
          </button>
        </div>
      </div>
    </header>
  );
}
