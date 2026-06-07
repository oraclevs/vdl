import { useToast } from '../lib/ToastContext';
import { isFinished, PLATFORM_LABELS, shortenUrl, STATUS_INFO } from '../lib/format';

const TONE_CLASSES = {
  ink: { bar: 'bg-ink-faint', badge: 'bg-paper-deep text-ink-soft' },
  coral: { bar: 'bg-coral', badge: 'bg-coral-soft text-coral' },
  teal: { bar: 'bg-teal', badge: 'bg-teal-soft text-teal' },
  rose: { bar: 'bg-rose', badge: 'bg-rose-soft text-rose' },
  plum: { bar: 'bg-plum', badge: 'bg-plum-soft text-plum' },
};

export default function SessionCard({ session, onCancel, onDismiss }) {
  const toast = useToast();
  const info = STATUS_INFO[session.status] || { label: session.status, tone: 'ink' };
  const tone = TONE_CLASSES[info.tone] || TONE_CLASSES.ink;
  const finished = isFinished(session.status);
  const pct = Math.max(0, Math.min(100, session.progress || 0));
  const indeterminate = !finished && pct <= 0;
  const failed = session.status === 'error' || session.status === 'cancelled';

  async function handleCancel() {
    try {
      await onCancel(session.id);
      toast('Asked the server to stop this download.', 'info');
    } catch {
      toast('Could not cancel — it may have already finished.', 'error');
    }
  }

  return (
    <li className="animate-rise overflow-hidden rounded-2xl border border-line bg-card shadow-sm shadow-ink/5">
      <div className="flex gap-4 p-5">
        <div className="hidden h-20 w-32 shrink-0 overflow-hidden rounded-xl bg-paper-deep sm:block">
          {session.thumbnail ? (
            <img src={session.thumbnail} alt="" className="h-full w-full object-cover" />
          ) : (
            <div className="flex h-full items-center justify-center text-[10px] uppercase tracking-widest text-ink-faint">
              {PLATFORM_LABELS[session.platform] || session.platform}
            </div>
          )}
        </div>

        <div className="min-w-0 flex-1">
          <div className="mb-1.5 flex flex-wrap items-center gap-2">
            <span className="rounded-full bg-paper-deep px-2.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-ink-soft">
              {PLATFORM_LABELS[session.platform] || session.platform}
            </span>
            <span className={`rounded-full px-2.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider ${tone.badge}`}>
              {info.label}
              {session.status === 'downloading' && (
                <span className="ml-1.5 inline-block h-1.5 w-1.5 animate-blink rounded-full bg-current align-middle" />
              )}
            </span>
            <span className="truncate text-xs text-ink-faint" title={session.url}>
              {shortenUrl(session.url)}
            </span>
          </div>

          <p className="truncate text-sm font-medium text-ink">
            {session.title || 'Fetching details…'}
            {session.uploader && <span className="font-normal text-ink-faint"> · {session.uploader}</span>}
          </p>

          <div className="mt-3 flex items-center gap-3">
            <div className="h-2 flex-1 overflow-hidden rounded-full bg-paper-deep">
              {indeterminate ? (
                <div className={`h-full w-1/3 animate-loading-sweep rounded-full ${failed ? 'bg-ink-faint/50' : tone.bar}`} />
              ) : (
                <div
                  className={`h-full rounded-full transition-[width] duration-500 ease-out ${tone.bar}`}
                  style={{ width: `${pct}%` }}
                />
              )}
            </div>
            <span className="w-14 text-right font-display text-lg italic text-ink tabular-nums">
              {indeterminate ? '···' : `${pct.toFixed(pct > 0 && pct < 10 ? 1 : 0)}%`}
            </span>
          </div>

          <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-ink-faint">
            {session.speed && <span>{session.speed}</span>}
            {session.eta && <span>ETA {session.eta}</span>}
            {session.size && <span>{session.size}</span>}
          </div>

          {session.outputPath && (
            <p className="mt-2 truncate text-xs text-teal" title={session.outputPath}>
              Saved to {session.outputPath}
            </p>
          )}
          {session.errorMessage && failed && (
            <p className="mt-2 text-xs text-rose">
              {session.status === 'cancelled' ? 'Cancelled — ' : 'Failed — '}
              {session.errorMessage}
            </p>
          )}
        </div>

        <div className="flex h-fit shrink-0 flex-col items-end gap-2">
          {!finished && (
            <button
              type="button"
              onClick={handleCancel}
              className="rounded-full border border-line px-3.5 py-1.5 text-xs font-medium text-ink-soft transition hover:border-rose/40 hover:text-rose"
            >
              Cancel
            </button>
          )}
          {finished && onDismiss && (
            <button
              type="button"
              onClick={() => onDismiss(session.id)}
              className="rounded-full border border-line px-3.5 py-1.5 text-xs font-medium text-ink-soft transition hover:border-ink/30 hover:text-ink"
            >
              Dismiss
            </button>
          )}
        </div>
      </div>
    </li>
  );
}
