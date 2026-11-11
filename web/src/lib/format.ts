// Mise en forme française des nombres, tailles et dates.

const nfCache = new Map<number, Intl.NumberFormat>();
export function nf(n: number, d = 0): string {
  if (!nfCache.has(d)) nfCache.set(d, new Intl.NumberFormat('fr-FR', { minimumFractionDigits: d, maximumFractionDigits: d }));
  return nfCache.get(d)!.format(n);
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

export function initials(name: string): string {
  return name.split(/[\s.@_-]+/).filter(Boolean).slice(0, 2).map((p) => p[0]!.toUpperCase()).join('');
}
