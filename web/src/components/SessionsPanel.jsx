import SessionCard from './SessionCard';
import { isFinished } from '../lib/format';

export default function SessionsPanel({ sessions, onCancel, onDismiss }) {
  const active = sessions.filter((s) => !isFinished(s.status)).length;

  return (
    <section id="sessions" className="border-t border-line/70 bg-paper-deep/40">
      <div className="mx-auto max-w-6xl px-6 py-14 sm:py-20">
        <div className="mb-8 flex flex-wrap items-end justify-between gap-3">
          <div>
            <h2 className="font-display text-4xl italic text-ink">Your sessions</h2>
            <p className="mt-2 text-ink-soft">Everything you've downloaded this session, with live progress.</p>
          </div>
          <span className="rounded-full border border-line bg-card px-4 py-1.5 text-sm text-ink-soft">
            {active} active · {sessions.length} total
          </span>
        </div>

        {sessions.length === 0 ? (
          <div className="rounded-3xl border border-dashed border-line bg-card/60 px-8 py-16 text-center">
            <p className="font-display text-2xl italic text-ink-soft">Nothing here yet</p>
            <p className="mt-2 text-sm text-ink-faint">
              Preview a link above and hit download — it'll show up here with live progress.
            </p>
          </div>
        ) : (
          <ul className="flex flex-col gap-4">
            {sessions.map((session) => (
              <SessionCard key={session.id} session={session} onCancel={onCancel} onDismiss={onDismiss} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
