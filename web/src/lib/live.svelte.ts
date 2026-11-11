// Temps réel : une connexion WebSocket par onglet, des salles, la présence.
// Les écritures passent par l'API REST ; ce canal ne fait que prévenir.

import { CLIENT_ID } from './api';

export interface PresenceMember { id: string; name: string; color: string }
export interface Change { type: 'change'; room: string; kind: string; id: string; action: string; rev: number | null; actor: string; client: string | null }
type Handler = (c: Change) => void;

export const live = $state<{ connected: boolean; presence: Record<string, PresenceMember[]> }>({ connected: false, presence: {} });

let ws: WebSocket | null = null;
let wanted = new Map<string, Set<Handler>>();
let retry = 1000;
let beat: ReturnType<typeof setInterval> | null = null;
let closedByUs = false;

function send(m: object) {
  if (ws?.readyState === WebSocket.OPEN) ws.send(JSON.stringify(m));
}

function connect() {
  if (ws || typeof WebSocket === 'undefined') return;
  closedByUs = false;
  const url = `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/api/live`;
  ws = new WebSocket(url);
  ws.onopen = () => {
    live.connected = true;
    retry = 1000;
    for (const room of wanted.keys()) send({ type: 'join', room });
    // Après une coupure, chaque salle recharge ce qui a pu changer.
    for (const [room, hs] of wanted) hs.forEach((h) => h({ type: 'change', room, kind: 'resync', id: '*', action: 'resync', rev: null, actor: '', client: null }));
    beat = setInterval(() => send({ type: 'ping' }), 30_000);
  };
  ws.onmessage = (e) => {
    let m: any;
    try { m = JSON.parse(e.data); } catch { return; }
    if (m.type === 'presence') live.presence[m.room] = m.members;
    else if (m.type === 'change') {
      if (m.client === CLIENT_ID) return;
      wanted.get(m.room)?.forEach((h) => h(m));
    } else if (m.type === 'resync') {
      for (const [room, hs] of wanted) hs.forEach((h) => h({ type: 'change', room, kind: 'resync', id: '*', action: 'resync', rev: null, actor: '', client: null }));
    }
  };
  ws.onclose = () => {
    ws = null;
    live.connected = false;
    if (beat) clearInterval(beat);
    if (!closedByUs && wanted.size) setTimeout(connect, retry);
    retry = Math.min(retry * 2, 15_000);
  };
}

/** Rejoint une salle tant que le composant vit ; renvoie la fonction pour la quitter. */
export function join(room: string, handler: Handler): () => void {
  const first = !wanted.has(room);
  if (first) wanted.set(room, new Set());
  wanted.get(room)!.add(handler);
  if (!ws) connect();
  else if (first) send({ type: 'join', room });
  return () => {
    const hs = wanted.get(room);
    hs?.delete(handler);
    if (hs && hs.size === 0) {
      wanted.delete(room);
      send({ type: 'leave', room });
      delete live.presence[room];
    }
    if (!wanted.size && ws) { closedByUs = true; ws.close(); }
  };
}
