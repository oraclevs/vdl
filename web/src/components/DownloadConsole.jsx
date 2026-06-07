import { useState } from 'react';
import { api, ApiError } from '../lib/api';
import { useToast } from '../lib/ToastContext';
import PlatformTabs from './PlatformTabs';
import MetadataPreview from './MetadataPreview';
import { Field, Select, TextInput, Toggle } from './Field';

const QUALITIES = ['best', '1080', '720', '480', '360', 'worst'];
const FORMATS = ['mp4', 'mkv', 'webm', 'mp3', 'm4a', 'opus'];
const AUDIO_FORMATS = new Set(['mp3', 'm4a', 'opus']);

export default function DownloadConsole({ onLaunched }) {
  const toast = useToast();
  const [platform, setPlatform] = useState('yt');
  const [url, setUrl] = useState('');
  const [quality, setQuality] = useState('');
  const [format, setFormat] = useState('');
  const [outputDir, setOutputDir] = useState('');
  const [audioOnly, setAudioOnly] = useState(false);
  const [videoOnly, setVideoOnly] = useState(false);
  const [meta, setMeta] = useState(null);
  const [error, setError] = useState(null);
  const [previewing, setPreviewing] = useState(false);
  const [launching, setLaunching] = useState(false);

  const audioForced = AUDIO_FORMATS.has(format);

  function changePlatform(next) {
    setPlatform(next);
    setMeta(null);
    setError(null);
  }

  function changeFormat(next) {
    setFormat(next);
    if (AUDIO_FORMATS.has(next)) {
      setAudioOnly(true);
      setVideoOnly(false);
    }
  }

  function setAudio(next) {
    setAudioOnly(next);
    if (next) setVideoOnly(false);
  }

  function setVideo(next) {
    setVideoOnly(next);
    if (next) setAudioOnly(false);
  }

  async function fetchPreview() {
    const trimmed = url.trim();
    setError(null);
    if (!trimmed) {
      setError('Paste a link first — that\'s how we know what to fetch.');
      return;
    }
    setPreviewing(true);
    setMeta(null);
    try {
      const data = await api('POST', '/api/fetch-metadata', { url: trimmed, platform });
      setMeta(data);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Something went wrong fetching that link.');
    } finally {
      setPreviewing(false);
    }
  }

  async function startDownload() {
    const trimmed = url.trim();
    if (!trimmed) return;
    setLaunching(true);
    try {
      const data = await api('POST', '/api/download', {
        platform,
        url: trimmed,
        quality: quality || null,
        format: format || null,
        audio_only: audioOnly,
        video_only: videoOnly,
        start: null,
        end: null,
        output_dir: outputDir.trim() || null,
      });
      onLaunched(data.session_id, { url: trimmed, platform });
      toast('Download started — track its progress in Sessions below.', 'success');
      document.getElementById('sessions')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Could not start the download.');
    } finally {
      setLaunching(false);
    }
  }

  function handleDisabledPlatform(p) {
    toast(`${p.label} downloads aren't available from the browser yet — use the vdl CLI for now.`, 'warn');
  }

  function clearAll() {
    setUrl('');
    setQuality('');
    setFormat('');
    setOutputDir('');
    setAudioOnly(false);
    setVideoOnly(false);
    setMeta(null);
    setError(null);
  }

  return (
    <section id="download" className="mx-auto max-w-6xl px-6 py-14 sm:py-20">
      <div className="mb-8 max-w-xl">
        <h2 className="font-display text-4xl italic text-ink">Start a download</h2>
        <p className="mt-2 text-ink-soft">
          Choose where the link comes from, paste it in, and preview before you commit.
        </p>
      </div>

      <div className="rounded-3xl border border-line bg-card p-6 shadow-sm shadow-ink/5 sm:p-8">
        <div className="space-y-6">
          <div>
            <p className="mb-2.5 text-xs font-semibold uppercase tracking-[0.16em] text-ink-faint">
              Where is it from?
            </p>
            <PlatformTabs value={platform} onChange={changePlatform} onDisabledClick={handleDisabledPlatform} />
          </div>

          <div className="flex flex-col gap-3 sm:flex-row sm:items-end">
            <div className="flex-1">
              <Field label="Link to download">
                <TextInput
                  type="url"
                  placeholder="Paste a video, post, or track URL…"
                  value={url}
                  onChange={(e) => { setUrl(e.target.value); setError(null); }}
                  onKeyDown={(e) => { if (e.key === 'Enter') fetchPreview(); }}
                  spellCheck={false}
                />
              </Field>
            </div>
            <button
              type="button"
              onClick={fetchPreview}
              disabled={previewing}
              className="rounded-xl border border-ink/15 bg-ink px-6 py-2.5 text-sm font-semibold text-paper transition hover:bg-ink/90 disabled:cursor-wait disabled:opacity-60"
            >
              {previewing ? 'Looking it up…' : 'Preview'}
            </button>
          </div>

          {error && (
            <p className="animate-rise rounded-2xl border border-rose/30 bg-rose-soft px-4 py-3 text-sm text-rose">
              {error}
            </p>
          )}

          <MetadataPreview meta={meta} />

          <div className="grid gap-4 sm:grid-cols-3">
            <Field label="Quality" hint="Leave blank to use your saved default">
              <Select value={quality} onChange={(e) => setQuality(e.target.value)} options={QUALITIES} placeholder="Platform default" />
            </Field>
            <Field label="File format" hint="Picking mp3/m4a/opus switches to audio-only">
              <Select value={format} onChange={(e) => changeFormat(e.target.value)} options={FORMATS} placeholder="Platform default" />
            </Field>
            <Field label="Save to" hint="Leave blank for your configured downloads folder">
              <TextInput placeholder="e.g. ~/Movies" value={outputDir} onChange={(e) => setOutputDir(e.target.value)} />
            </Field>
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <Toggle
              checked={audioOnly}
              onChange={setAudio}
              label="Audio only"
              sub="Skip the video stream entirely"
              disabled={audioForced}
            />
            <Toggle
              checked={videoOnly}
              onChange={setVideo}
              label="Video only"
              sub="Skip the audio stream entirely"
              disabled={audioForced}
            />
          </div>

          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-line pt-6">
            <button
              type="button"
              onClick={clearAll}
              className="text-sm font-medium text-ink-faint transition hover:text-ink-soft"
            >
              Clear form
            </button>
            <button
              type="button"
              onClick={startDownload}
              disabled={!meta || launching}
              className="rounded-xl bg-coral px-8 py-3 text-sm font-semibold text-white shadow-md shadow-coral/30 transition hover:bg-coral/90 disabled:cursor-not-allowed disabled:bg-line disabled:text-ink-faint disabled:shadow-none"
            >
              {launching ? 'Starting…' : 'Download'}
            </button>
          </div>
          {!meta && !error && (
            <p className="text-right text-xs text-ink-faint">Preview a link to unlock the download button.</p>
          )}
        </div>
      </div>
    </section>
  );
}
