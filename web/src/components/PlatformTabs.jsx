const PLATFORMS = [
  { id: 'yt', label: 'YouTube' },
  { id: 'tk', label: 'TikTok' },
  { id: 'ig', label: 'Instagram' },
  { id: 'tw', label: 'Twitter / X' },
  { id: 'sp', label: 'Spotify', disabled: true, note: 'CLI only, for now' },
];

export default function PlatformTabs({ value, onChange, onDisabledClick }) {
  return (
    <div className="flex flex-wrap gap-2" role="tablist" aria-label="Platform">
      {PLATFORMS.map((p) => {
        const active = p.id === value;
        return (
          <button
            key={p.id}
            type="button"
            role="tab"
            aria-selected={active}
            disabled={p.disabled && false /* keep clickable to surface the explainer toast */}
            onClick={() => (p.disabled ? onDisabledClick(p) : onChange(p.id))}
            className={[
              'rounded-full px-4 py-2 text-sm font-medium transition border',
              active
                ? 'border-coral bg-coral text-white shadow-sm shadow-coral/30'
                : p.disabled
                  ? 'border-line/70 bg-card text-ink-faint hover:border-line'
                  : 'border-line bg-card text-ink-soft hover:border-ink/30 hover:text-ink',
            ].join(' ')}
          >
            {p.label}
            {p.note && <span className="ml-1.5 text-[10px] uppercase tracking-wide opacity-70">{p.note}</span>}
          </button>
        );
      })}
    </div>
  );
}
