// Mise en forme française des nombres, tailles et dates.

const nfCache = new Map<number, Intl.NumberFormat>();
export function nf(n: number, d = 0): string {
  if (!nfCache.has(d)) nfCache.set(d, new Intl.NumberFormat('fr-FR', { minimumFractionDigits: d, maximumFractionDigits: d }));
  return nfCache.get(d)!.format(n);
}

/** Marque du pluriel français : « s » (ou `many`) au-delà de 1, rien (ou `one`) sinon (0 et 1 au singulier). */
export function pl(n: number, many = 's', one = ''): string {
  return n > 1 ? many : one;
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} o`;
  const u = ['Ko', 'Mo', 'Go', 'To'];
  let v = n / 1024, i = 0;
  while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
  return `${nf(v, v < 10 ? 1 : 0)} ${u[i]}`;
}

export function dateTime(iso: string | null | undefined): string {
  if (!iso) return '—';
  const d = new Date(iso);
  if (isNaN(+d)) return iso;
  return d.toLocaleString('fr-FR', { day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit' });
}

export function relative(iso: string | null | undefined): string {
  if (!iso) return 'jamais';
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 60) return 'à l’instant';
  if (s < 3600) return `il y a ${Math.round(s / 60)} min`;
  if (s < 86400) return `il y a ${Math.round(s / 3600)} h`;
  if (s < 86400 * 7) return `il y a ${Math.round(s / 86400)} j`;
  return dateTime(iso);
}

/** Jour en titre de groupe : « Aujourd’hui », « Hier », sinon « Samedi 14 novembre 2026 ». */
export function dayTitle(iso: string): string {
  const d = new Date(iso);
  if (isNaN(+d)) return iso;
  const day = (x: Date) => x.toDateString();
  const now = new Date();
  if (day(d) === day(now)) return 'Aujourd’hui';
  if (day(d) === day(new Date(now.getTime() - 86400000))) return 'Hier';
  const s = d.toLocaleDateString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Heure seule : « 07:03 ». */
export function timeOf(iso: string): string {
  const d = new Date(iso);
  return isNaN(+d) ? iso : d.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });
}

/** Regroupe des entrées datées par jour local, dans l'ordre reçu. */
export function byDay<T extends { at: string }>(list: T[]): { key: string; title: string; items: T[] }[] {
  const out: { key: string; title: string; items: T[] }[] = [];
  for (const e of list) {
    const key = new Date(e.at).toDateString();
    const last = out[out.length - 1];
    if (last && last.key === key) last.items.push(e); else out.push({ key, title: dayTitle(e.at), items: [e] });
  }
  return out;
}

export function initials(name: string): string {
  return name.split(/[\s.@_-]+/).filter(Boolean).slice(0, 2).map((p) => p[0]!.toUpperCase()).join('');
}
