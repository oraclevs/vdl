import { formatDuration } from '../lib/format';

export default function MetadataPreview({ meta }) {
  if (!meta) return null;
  const duration = formatDuration(meta.duration);

  return (
    <div className="animate-rise overflow-hidden rounded-3xl border border-line bg-card shadow-sm shadow-ink/5">
      <div className="grid gap-0 sm:min-h-[24rem] sm:grid-cols-[24rem_1fr]">
        <div className="aspect-video bg-paper-deep sm:aspect-auto sm:h-full">
          {meta.thumbnail ? (
            <img
              src={meta.thumbnail}
              alt={meta.title}
              className="h-full w-full object-cover"
            />
          ) : (
            <div className="flex h-full min-h-[16rem] items-center justify-center text-xs uppercase tracking-[0.2em] text-ink-faint">
              No preview image
            </div>
          )}
        </div>
        <div className="flex flex-col justify-center gap-4 p-8 sm:p-10">
          <p className="text-xs font-semibold uppercase tracking-[0.3em] text-coral">
            Here's what we found
          </p>
          <h3 className="font-display text-4xl italic leading-tight text-ink sm:text-5xl">{meta.title}</h3>
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-base text-ink-soft">
            {meta.uploader && <span className="font-medium text-ink">{meta.uploader}</span>}
            {meta.uploader && duration && <span className="text-ink-faint">·</span>}
            {duration && <span>{duration}</span>}
            {(meta.uploader || duration) && <span className="text-ink-faint">·</span>}
            <span>{meta.formats.length} formats available</span>
          </div>
          <p className="max-w-md text-sm leading-relaxed text-ink-soft">
            Looks right? Pick your quality and format below, then hit download — we'll
            stream the file straight to your downloads folder and show you live progress.
          </p>
        </div>
      </div>
    </div>
  );
}
