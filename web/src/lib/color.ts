// Couleurs choisies par l'utilisateur : une teinte quasi noire disparaît sur le thème sombre ;
// on lui substitue la couleur du texte (noire en clair, claire en sombre).
function luminance(c: string): number | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(c.trim());
  if (!m) return null;
  const n = parseInt(m[1]!, 16);
  const lin = (v: number) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
  return 0.2126 * lin(n >> 16) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
}
const tooDark = (c: string) => { const l = luminance(c); return l != null && l < 0.03; };

/** Pastille de couleur (rôle…) : une teinte quasi noire prend la couleur du texte. */
export function visibleColor(c: string | null | undefined, fallback = 'var(--fg-3)'): string {
  if (!c) return fallback;
  return tooDark(c) ? 'var(--fg)' : c;
}

/** Fond d'avatar (texte blanc) : une teinte quasi noire devient un ardoise lisible sur les deux thèmes. */
export function avatarColor(c: string): string {
  return tooDark(c) ? '#4A5266' : c;
}
