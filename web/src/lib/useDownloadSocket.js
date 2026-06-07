import { useEffect, useRef, useState } from 'react';

/**
 * Owns the single WebSocket connection: reconnects with backoff, resubscribes
 * to every previously-subscribed session id on reconnect, sends heartbeat
 * pings, and forwards parsed server events to `onEvent`.
 */
export function useDownloadSocket(onEvent) {
  const [connection, setConnection] = useState('connecting');
  const wsRef = useRef(null);
  const subscribedRef = useRef(new Set());
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  useEffect(() => {
    let cancelled = false;
    let socket = null;
    let reconnectDelay = 1000;
    let reconnectTimer = null;
    let heartbeat = null;

    function send(message) {
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify(message));
      }
    }

    function connect() {
      if (cancelled) return;
      const proto = location.protocol === 'https:' ? 'wss' : 'ws';
      socket = new WebSocket(`${proto}://${location.host}/ws`);
      wsRef.current = socket;

      socket.addEventListener('open', () => {
        reconnectDelay = 1000;
        setConnection('live');
        subscribedRef.current.forEach((id) => send({ type: 'subscribe', session_id: id }));
        heartbeat = setInterval(() => send({ type: 'ping' }), 25000);
      });

      socket.addEventListener('message', (event) => {
        try {
          const message = JSON.parse(event.data);
          if (message.type !== 'pong') onEventRef.current(message);
        } catch {
          /* ignore malformed frames */
        }
      });

      socket.addEventListener('close', () => {
        if (cancelled) return;
        setConnection('offline');
        clearInterval(heartbeat);
        reconnectTimer = setTimeout(connect, reconnectDelay);
        reconnectDelay = Math.min(reconnectDelay * 1.6, 15000);
      });

      socket.addEventListener('error', () => socket.close());
    }

    connect();

    return () => {
      cancelled = true;
      clearTimeout(reconnectTimer);
      clearInterval(heartbeat);
      socket?.close();
    };
  }, []);

  function subscribe(sessionId) {
    subscribedRef.current.add(sessionId);
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ type: 'subscribe', session_id: sessionId }));
    }
  }

  return { connection, subscribe };
}
