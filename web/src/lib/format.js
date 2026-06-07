export function shortenUrl(url) {
  try {
    const u = new URL(url);
    const path = u.pathname.length > 30 ? u.pathname.slice(0, 30) + '…' : u.pathname;
    return u.hostname.replace(/^www\./, '') + path;
  } catch {
    return url.length > 44 ? url.slice(0, 44) + '…' : url;
  }
}

export function formatDuration(totalSeconds) {
  if (totalSeconds == null) return null;
  const s = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return h > 0
    ? `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`
    : `${m}:${String(sec).padStart(2, '0')}`;
}

export function isFinished(status) {
  return status === 'complete' || status === 'error' || status === 'cancelled';
}

export const PLATFORM_LABELS = {
  yt: 'YouTube',
  tk: 'TikTok',
  ig: 'Instagram',
  tw: 'Twitter / X',
  sp: 'Spotify',
};

export const STATUS_INFO = {
  pending: { label: 'Queued', tone: 'ink' },
  fetching_metadata: { label: 'Reading info', tone: 'plum' },
  downloading: { label: 'Downloading', tone: 'coral' },
  merging: { label: 'Finishing up', tone: 'coral' },
  complete: { label: 'Done', tone: 'teal' },
  error: { label: 'Failed', tone: 'rose' },
  cancelled: { label: 'Cancelled', tone: 'ink' },
};
