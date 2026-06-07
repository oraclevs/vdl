import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from './api';
import { isFinished } from './format';
import { useDownloadSocket } from './useDownloadSocket';

function blank(id, patch) {
  return {
    id,
    url: '',
    platform: 'yt',
    status: 'pending',
    progress: 0,
    speed: null,
    eta: null,
    size: null,
    title: null,
    uploader: null,
    thumbnail: null,
    outputPath: null,
    errorMessage: null,
    ...patch,
  };
}

/**
 * Single source of truth for download sessions. Merges three inputs into one
 * map keyed by session id — React then diffs the resulting list, so progress
 * updates patch existing cards in place instead of replacing markup wholesale:
 *   - REST `POST /api/download` response (session just launched locally)
 *   - REST `GET /api/downloads` polling (authoritative status/progress/list)
 *   - WebSocket `ServerEvent` stream (live status/metadata/progress/complete/error)
 */
export function useSessions() {
  const [sessions, setSessions] = useState(new Map());
  const sessionsRef = useRef(sessions);
  sessionsRef.current = sessions;
  // Sessions the user dismissed locally — the server keeps finished sessions
  // around (capped at 50) so the next REST poll would otherwise resurrect them.
  const dismissedRef = useRef(new Set());

  const patchSession = useCallback((id, patch) => {
    if (dismissedRef.current.has(id)) return;
    setSessions((prev) => {
      const next = new Map(prev);
      const existing = next.get(id) || blank(id);
      next.set(id, { ...existing, ...patch });
      return next;
    });
  }, []);

  const handleEvent = useCallback(
    (message) => {
      switch (message.type) {
        case 'status':
          patchSession(message.session_id, { status: message.status });
          break;
        case 'metadata':
          patchSession(message.session_id, {
            title: message.title,
            uploader: message.uploader,
            thumbnail: message.thumbnail,
            duration: message.duration,
          });
          break;
        case 'progress':
          patchSession(message.session_id, {
            progress: message.percent,
            speed: message.speed,
            eta: message.eta,
            size: message.size,
          });
          break;
        case 'complete':
          patchSession(message.session_id, {
            status: 'complete',
            progress: 100,
            outputPath: message.output_path,
          });
          break;
        case 'error':
          patchSession(message.session_id, {
            status: message.message === 'cancelled by user' ? 'cancelled' : 'error',
            errorMessage: message.message,
          });
          break;
        default:
          break;
      }
    },
    [patchSession],
  );

  const { connection, subscribe } = useDownloadSocket(handleEvent);

  const addLaunched = useCallback(
    (id, patch) => {
      setSessions((prev) => {
        if (prev.has(id)) return prev;
        const next = new Map(prev);
        next.set(id, blank(id, patch));
        return next;
      });
      subscribe(id);
    },
    [subscribe],
  );

  const refresh = useCallback(async () => {
    let list;
    try {
      list = await api('GET', '/api/downloads');
    } catch {
      return;
    }
    setSessions((prev) => {
      const next = new Map(prev);
      list.forEach((item) => {
        if (dismissedRef.current.has(item.session_id)) return;
        const existing = next.get(item.session_id);
        next.set(item.session_id, {
          ...(existing || blank(item.session_id)),
          url: item.url,
          platform: item.platform,
          status: item.status,
          progress: item.progress,
          errorMessage: item.error_message ?? (existing ? existing.errorMessage : null),
        });
        if (!isFinished(item.status)) subscribe(item.session_id);
      });
      return next;
    });
  }, [subscribe]);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 8000);
    return () => clearInterval(interval);
  }, [refresh]);

  const cancel = useCallback(async (id) => {
    await api('DELETE', `/api/download/${id}`);
  }, []);

  const dismiss = useCallback((id) => {
    dismissedRef.current.add(id);
    setSessions((prev) => {
      if (!prev.has(id)) return prev;
      const next = new Map(prev);
      next.delete(id);
      return next;
    });
  }, []);

  const list = Array.from(sessions.values()).sort((a, b) => {
    const aFinished = isFinished(a.status);
    const bFinished = isFinished(b.status);
    if (aFinished !== bFinished) return aFinished ? 1 : -1;
    return 0;
  });

  return { sessions: list, connection, addLaunched, cancel, dismiss };
}
