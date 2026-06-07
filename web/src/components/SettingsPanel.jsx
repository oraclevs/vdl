import { useEffect, useState } from 'react';
import { api, ApiError } from '../lib/api';
import { useToast } from '../lib/ToastContext';
import { Field, Select, TextInput, Toggle } from './Field';

const QUALITIES = ['best', '1080', '720', '480', '360', 'worst'];
const FORMATS = ['mp4', 'mkv', 'webm', 'mp3', 'm4a', 'opus'];
const PLATFORM_KEYS = ['youtube', 'tiktok', 'instagram', 'twitter', 'spotify'];

export default function SettingsPanel({ open, onClose }) {
  const toast = useToast();
  const [config, setConfig] = useState(null);
  const [draft, setDraft] = useState(null);
  const [loadError, setLoadError] = useState(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open || config) return;
    api('GET', '/api/config')
      .then((data) => {
        setConfig(data);
        setDraft(data);
      })
      .catch((err) => setLoadError(err instanceof ApiError ? err.message : 'Could not load settings.'));
  }, [open, config]);

  useEffect(() => {
    function onKey(e) {
      if (e.key === 'Escape') onClose();
    }
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  if (!open) return null;

  function patch(path, value) {
    setDraft((prev) => {
      const next = structuredClone(prev);
      let target = next;
      for (let i = 0; i < path.length - 1; i++) target = target[path[i]];
      target[path[path.length - 1]] = value;
      return next;
    });
  }

  function setCookiesFile(value) {
    setDraft((prev) => ({ ...prev, cookies_file: value || null, cookies_from_browser: value ? null : prev.cookies_from_browser }));
  }
  function setCookiesBrowser(value) {
    setDraft((prev) => ({ ...prev, cookies_from_browser: value || null, cookies_file: value ? null : prev.cookies_file }));
  }

  async function save() {
    if (draft.cookies_file && draft.cookies_from_browser) {
      toast('Cookies file and browser extraction can\'t both be set — clear one first.', 'error');
      return;
    }
    setSaving(true);
    try {
      const saved = await api('PUT', '/api/config', draft);
      setConfig(saved);
      setDraft(saved);
      toast('Settings saved to ~/.config/vdl/config.yaml', 'success');
    } catch (err) {
      toast(err instanceof ApiError ? err.message : 'Could not save settings.', 'error');
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <button
        type="button"
        aria-label="Close settings"
        onClick={onClose}
        className="absolute inset-0 bg-ink/30 backdrop-blur-[2px]"
      />
      <aside className="relative flex h-full w-full max-w-lg flex-col overflow-hidden border-l border-line bg-paper shadow-2xl animate-rise">
        <div className="flex items-center justify-between border-b border-line px-7 py-5">
          <div>
            <h2 className="font-display text-3xl italic text-ink">Settings</h2>
            <p className="text-xs text-ink-faint">Saved straight to your vdl config file</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-full border border-line px-4 py-2 text-sm text-ink-soft transition hover:border-ink/30 hover:text-ink"
          >
            Close
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-7 py-6">
          {loadError && <p className="rounded-2xl border border-rose/30 bg-rose-soft px-4 py-3 text-sm text-rose">{loadError}</p>}
          {!draft && !loadError && <p className="text-sm text-ink-faint">Loading your current configuration…</p>}

          {draft && (
            <div className="space-y-6">
              <Field label="Downloads folder">
                <TextInput value={draft.download_path} onChange={(e) => patch(['download_path'], e.target.value)} />
              </Field>

              <div className="grid gap-4 sm:grid-cols-2">
                <Field label="Default format">
                  <Select value={draft.default_format} onChange={(e) => patch(['default_format'], e.target.value)} options={FORMATS} />
                </Field>
                <Field label="Default quality">
                  <Select value={draft.default_video_quality} onChange={(e) => patch(['default_video_quality'], e.target.value)} options={QUALITIES} />
                </Field>
              </div>

              <Field label="Search results to show" hint="How many results the interactive YouTube search returns">
                <TextInput
                  type="number"
                  min="1"
                  max="50"
                  value={draft.search_results_count}
                  onChange={(e) => patch(['search_results_count'], Number(e.target.value) || draft.search_results_count)}
                />
              </Field>

              <div>
                <p className="mb-2.5 text-xs font-semibold uppercase tracking-[0.16em] text-ink-faint">
                  Per-platform quality
                </p>
                <div className="grid gap-3 rounded-2xl border border-line bg-card p-4 sm:grid-cols-2">
                  {PLATFORM_KEYS.map((key) => (
                    <Field key={key} label={key}>
                      <Select
                        value={draft.platform_quality[key]}
                        onChange={(e) => patch(['platform_quality', key], e.target.value)}
                        options={QUALITIES}
                      />
                    </Field>
                  ))}
                </div>
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <Field label="Cookies file" hint="Mutually exclusive with browser extraction">
                  <TextInput
                    placeholder="/path/to/cookies.txt"
                    value={draft.cookies_file || ''}
                    onChange={(e) => setCookiesFile(e.target.value)}
                  />
                </Field>
                <Field label="Cookies from browser" hint="e.g. firefox, chrome">
                  <TextInput
                    placeholder="firefox"
                    value={draft.cookies_from_browser || ''}
                    onChange={(e) => setCookiesBrowser(e.target.value)}
                  />
                </Field>
              </div>

              <div className="grid gap-3 sm:grid-cols-2">
                <Toggle
                  checked={draft.confirm_before_download}
                  onChange={(v) => patch(['confirm_before_download'], v)}
                  label="Confirm before download"
                  sub="Applies to the command-line interface"
                />
                <Toggle
                  checked={draft.no_progress}
                  onChange={(v) => patch(['no_progress'], v)}
                  label="Plain progress output"
                  sub="Disables animated bars in the CLI"
                />
                <Toggle
                  checked={draft.termux_mode}
                  onChange={(v) => patch(['termux_mode'], v)}
                  label="Termux mode"
                  sub="For Android/Termux installs"
                />
              </div>

              <Field label="Helper binaries folder" hint="Advanced — only change this if you know what it does">
                <TextInput value={draft.bins_dir} onChange={(e) => patch(['bins_dir'], e.target.value)} />
              </Field>
            </div>
          )}
        </div>

        {draft && (
          <div className="flex items-center justify-end gap-3 border-t border-line px-7 py-5">
            <button
              type="button"
              onClick={() => setDraft(config)}
              className="text-sm font-medium text-ink-faint transition hover:text-ink-soft"
            >
              Revert changes
            </button>
            <button
              type="button"
              onClick={save}
              disabled={saving}
              className="rounded-xl bg-ink px-6 py-2.5 text-sm font-semibold text-paper transition hover:bg-ink/90 disabled:cursor-wait disabled:opacity-60"
            >
              {saving ? 'Saving…' : 'Save settings'}
            </button>
          </div>
        )}
      </aside>
    </div>
  );
}
