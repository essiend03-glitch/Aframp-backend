// Issue #479 — Real-Time Data Streaming & WebSocket Client
// Persistent WebSocket hook with heartbeat, auto-reconnect, and React Query cache integration.

import { useEffect, useRef, useCallback, useState } from 'react';

export type WSReadyState = 'connecting' | 'open' | 'reconnecting' | 'closed';

interface UseWebSocketOptions {
  url: string;
  onMessage: (event: MessageEvent) => void;
  heartbeatIntervalMs?: number;
  reconnectDelayMs?: number;
  maxReconnectAttempts?: number;
}

export function useWebSocket({
  url,
  onMessage,
  heartbeatIntervalMs = 30_000,
  reconnectDelayMs = 2_000,
  maxReconnectAttempts = 10,
}: UseWebSocketOptions) {
  const wsRef = useRef<WebSocket | null>(null);
  const heartbeatRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const reconnectRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const attemptsRef = useRef(0);
  const [readyState, setReadyState] = useState<WSReadyState>('connecting');

  const clearTimers = useCallback(() => {
    if (heartbeatRef.current) clearInterval(heartbeatRef.current);
    if (reconnectRef.current) clearTimeout(reconnectRef.current);
  }, []);

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    setReadyState(attemptsRef.current > 0 ? 'reconnecting' : 'connecting');
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      attemptsRef.current = 0;
      setReadyState('open');
      // Start heartbeat ping
      heartbeatRef.current = setInterval(() => {
        if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: 'ping' }));
      }, heartbeatIntervalMs);
    };

    ws.onmessage = (evt) => {
      try {
        const data = JSON.parse(evt.data);
        if (data?.type === 'pong') return; // ignore heartbeat responses
      } catch { /* non-JSON frames pass through */ }
      onMessage(evt);
    };

    ws.onclose = () => {
      clearTimers();
      if (attemptsRef.current < maxReconnectAttempts) {
        attemptsRef.current += 1;
        setReadyState('reconnecting');
        const delay = Math.min(reconnectDelayMs * 2 ** attemptsRef.current, 30_000);
        reconnectRef.current = setTimeout(connect, delay);
      } else {
        setReadyState('closed');
      }
    };

    ws.onerror = () => ws.close();
  }, [url, onMessage, heartbeatIntervalMs, reconnectDelayMs, maxReconnectAttempts, clearTimers]);

  useEffect(() => {
    connect();
    return () => {
      clearTimers();
      wsRef.current?.close();
    };
  }, [connect, clearTimers]);

  const send = useCallback((data: unknown) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(data));
    }
  }, []);

  return { readyState, send };
}
