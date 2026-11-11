// Panels de brassage : mêmes règles que le moteur (crates/strata-domain/src/panel.rs).

import { refById } from './catalog.svelte';

export interface PanelSlot {
  index: number; connector: string; variant: string | null; label: string; notes: string | null;
  width: number; function: string | null; d_format: boolean;
}
export interface PanelGroup { row: number; start: number; len: number; label: string; color: string }
export interface PanelData {
  name: string; height_u: number; per_u: number; rear_depth_mm: number; notes: string | null;
  slots: PanelSlot[]; groups: PanelGroup[];
}
export interface PanelStats { total: number; used: number; free: number; fill_pct: number }
export type FillMode = 'one' | 'row' | 'all' | 'count';

export const PER_U = [8, 12, 16, 24];
export const D_FORMAT_MAX_PER_U = 16;

export const blankPanel = (name = ''): PanelData => ({ name, height_u: 1, per_u: 16, rear_depth_mm: 80, notes: null, slots: [], groups: [] });
export const clonePanel = (p: PanelData): PanelData => JSON.parse(JSON.stringify(p));

export const total = (p: PanelData) => p.height_u * p.per_u;
export const rowOf = (p: PanelData, i: number) => Math.floor((i - 1) / p.per_u) + 1;
export const colOf = (p: PanelData, i: number) => ((i - 1) % p.per_u) + 1;

export function slotAt(p: PanelData, i: number): PanelSlot | undefined {
  return p.slots.find((s) => i >= s.index && i < s.index + Math.max(1, s.width));
}

export function fits(p: PanelData, index: number, width: number, ignore?: PanelSlot): boolean {
  const w = Math.max(1, width);
  if (index < 1 || index + w - 1 > total(p)) return false;
  if (rowOf(p, index) !== rowOf(p, index + w - 1)) return false;
  return !p.slots.some((s) => s !== ignore && index < s.index + Math.max(1, s.width) && s.index < index + w);
}

/** Pose `proto` à partir de `start`, en sautant les emplacements occupés. Renvoie le nombre posé. */
export function fill(p: PanelData, start: number, mode: FillMode, proto: Omit<PanelSlot, 'index'>, count = 1): number {
  const w = Math.max(1, proto.width), t = total(p);
  if (start < 1 || start > t) return 0;
  const end = mode === 'row' ? rowOf(p, start) * p.per_u : t;
  const max = mode === 'one' ? 1 : mode === 'count' ? Math.max(1, count) : Infinity;
  let placed = 0, i = start;
  while (i <= end && placed < max) {
    if (i + w - 1 <= end && fits(p, i, w)) { p.slots.push({ ...proto, index: i }); placed++; i += w; } else i++;
  }
  p.slots.sort((a, b) => a.index - b.index);
  return placed;
}

export function moveSlot(p: PanelData, s: PanelSlot, to: number): boolean {
  if (!fits(p, to, s.width, s)) return false;
  s.index = to;
  p.slots.sort((a, b) => a.index - b.index);
  return true;
}

export function stats(p: PanelData): PanelStats {
  const t = total(p);
  const used = p.slots.reduce((n, s) => n + Math.max(1, s.width), 0);
  return { total: t, used, free: t - used, fill_pct: t ? Math.round((used * 100) / t) : 0 };
}

/** Le connecteur existe-t-il en format D, et la densité le permet-elle ? */
export const dFormatAllowed = (p: PanelData, connector: string) => p.per_u <= D_FORMAT_MAX_PER_U && !!refById('connectors', connector)?.data?.d_format;

export function slotColor(s: PanelSlot): string {
  return (s.function && refById('functions', s.function)?.data?.color) || refById('connectors', s.connector)?.data?.color || 'var(--fg-3)';
}

export function groupAt(p: PanelData, row: number, col: number): PanelGroup | undefined {
  return p.groups.find((g) => g.row === row && col >= g.start && col < g.start + g.len);
}

/** Famille de dessin d'un connecteur (voir ConnGlyph). */
export function family(c: string): string {
  if (c === 'bnc') return 'bnc';
  if (c === 'hd-bnc' || c === 'sma' || c === 'n-type') return 'hdbnc';
  if (c === 'din-1023') return 'din';
  if (c.startsWith('xlr')) return c;
  if (c === 'rj45' || c === 'ethercon') return 'rj45';
  if (c === 'hdmi' || c === 'displayport') return 'hdmi';
  if (c === 'usb') return 'usb';
  if (c === 'dsub9' || c === 'dsub25') return 'dsub';
  if (c.startsWith('opticalcon') || c === 'lc' || c === 'sc' || c === 'toslink') return 'fiber';
  if (c.startsWith('powercon') || c.startsWith('iec') || c.startsWith('cee') || c === 'schuko') return 'power';
  return 'round';
}
