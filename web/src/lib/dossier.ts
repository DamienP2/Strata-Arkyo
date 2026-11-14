// Dossier d'un projet (section 11.6) : documents assemblés dans un seul PDF — page de garde
// avec sommaire, sections à leur format, page de fin, pagination continue. Les données sont
// relues au moment de l'export ; un document sans donnée n'est jamais imprimé en page blanche.

import { mount, unmount } from 'svelte';
import { api } from './api';
import { fnColor, refList, refName, loadRefs } from './catalog.svelte';
import { addPage, drawTable, newDoc, paginate, projectHeader, size, svgPage, titleBlock, txt, type Col, type Doc, type PaperKey, type TitleBlock, type Vars } from './pdfkit';
import { dataUrl, drawingSvg, mapFloorSvg, synopticSvg, toSvgEl, type Rendered } from './render';
import { drawPV } from './pv';
import { withLightTheme, standalone } from './svgexport';
import { STATUS, type ProjectView } from './projects';

export interface DocDef { key: string; group: 'project' | 'prep' | 'network' | 'plans' | 'recette'; title: string; columns: string[] }
export interface DocOpt { key: string; include: boolean; paper: 'A4' | 'A3' | null; landscape: boolean | null; columns: string[]; hidden: string[]; title: string | null; pages?: string[] }
export interface Probe { state: 'ok' | 'empty' | 'error'; message: string; choices?: { id: string; label: string }[] }
export const GROUPS: Record<DocDef['group'], string> = { project: 'Projet', prep: 'Préparation', network: 'Réseau', plans: 'Plans', recette: 'Recette' };
/** La fiche et l'inventaire ont un format imposé. */
export const FIXED: Record<string, [PaperKey, boolean]> = { sheet: ['A4', false], inventory: ['A4', false], cover: ['A4', false] };
export const DEFAULT_FMT: Record<string, [PaperKey, boolean]> = {
  prep_cables: ['A4', false], prep_equipment: ['A4', false], prep_accessories: ['A4', false], prep_panels: ['A4', false], prep_adapters: ['A4', false], prep_containers: ['A4', false],
  net_ip: ['A4', true], net_2110: ['A4', true], net_dmx: ['A4', true], plan_racks: ['A3', true], plan_synoptic: ['A3', true], plan_maps: ['A3', true],
  plan_schematic: ['A3', true], plan_av: ['A3', true], plan_lighting: ['A3', true], recette: ['A4', false],
};
export const COL_LABEL: Record<string, string> = {
  element: 'Élément', nature: 'Nature', label: 'Désignation', quantity: 'Qté', tags: 'Repères', reserve: 'Réserve', supply: 'Appro.', status: 'Statut',
  equipment: 'Équipement', tag: 'Repère', port: 'Port', mode: 'Mode', vlan: 'VLAN', ip: 'Adresse IP', hostname: 'Nom d’hôte', redundancy: 'Réseau', peer: 'Relié à',
  flow: 'Flux', essence: 'Essence', address: 'Adresse', udp: 'UDP', fixture: 'Appareil', page: 'Page', console: 'N°', universe: 'Univers', channels: 'Canaux',
};
const COL_W: Record<string, number> = { element: 26, nature: 16, label: 50, quantity: 10, tags: 40, reserve: 12, supply: 16, status: 20, equipment: 32, tag: 14, port: 18, mode: 12, vlan: 18, ip: 22, hostname: 22, redundancy: 10, peer: 44, flow: 26, essence: 14, address: 22, udp: 12, fixture: 34, page: 24, console: 8, universe: 12, channels: 12 };

interface Ctx { view: ProjectView; cache: Map<string, any> }
async function get<T>(c: Ctx, path: string): Promise<T> { if (!c.cache.has(path)) c.cache.set(path, await api.get<T>(path)); return c.cache.get(path) as T; }
const pidOf = (c: Ctx) => c.view.project.id;

// ----- Données des tableaux -----

const SUPPLY: Record<string, string> = { stock: 'Stock', purchase: 'Achat', rental: 'Location' };
async function prepRows(c: Ctx, fam: string, prefix: string | null): Promise<Record<string, string>[]> {
  const p = await get<{ families: { key: string; statuses: { key: string; label: string }[]; lines: any[] }[] }>(c, `/projects/${pidOf(c)}/prep`);
  const f = p.families.find((x) => x.key === fam); if (!f) return [];
  return f.lines.filter((l) => !prefix || l.key.startsWith(prefix)).map((l) => ({
    label: `${l.label}${l.detail ? ` — ${l.detail}` : ''}`, quantity: String(l.total), reserve: String(l.reserve), supply: l.supply ? SUPPLY[l.supply] ?? l.supply : '—', status: f.statuses.find((s) => s.key === l.status)?.label ?? l.status,
  }));
}
async function tableRows(c: Ctx, key: string): Promise<Record<string, string>[]> {
  const v = c.view;
  switch (key) {
    case 'inventory': {
      const cname = (id: string | null) => v.containers.find((x) => x.id === id)?.name ?? 'Équipements libres';
      const nat: Record<string, string> = { equipment: 'Équipement', adapter: 'Adaptateur', accessory: 'Accessoire' };
      return v.articles.slice().sort((a, b) => cname(a.container_id).localeCompare(cname(b.container_id), 'fr')).map((a) => ({
        element: cname(a.container_id), nature: nat[a.nature] ?? a.nature, label: a.label, quantity: String(a.quantity),
        tags: v.instances.filter((i) => i.article_id === a.id).map((i) => i.tag).join(' '),
      }));
    }
    case 'prep_cables': return prepRows(c, 'cables', null);
    case 'prep_equipment': return prepRows(c, 'equipment', null);
    case 'prep_accessories': return prepRows(c, 'others', 'accessory:');
    case 'prep_panels': return prepRows(c, 'others', 'panel:');
    case 'prep_adapters': return prepRows(c, 'others', 'adapter:');
    case 'prep_containers': return prepRows(c, 'elements', null);
    case 'net_ip': {
      const n = await get<{ ports: any[]; vlans: any[] }>(c, `/projects/${pidOf(c)}/network`);
      const vn = (vid: number | null) => (vid == null ? '' : `${vid} ${n.vlans.find((x) => x.vid === vid)?.name ?? ''}`);
      return n.ports.map((p) => ({ element: p.element, equipment: p.equipment, tag: p.tag, port: p.port, mode: p.mode === 'trunk' ? 'Trunk' : 'Accès', vlan: p.mode === 'trunk' ? [p.vlan != null ? `natif ${p.vlan}` : '', ...p.vlans].filter(Boolean).join(', ') : vn(p.vlan), ip: p.ip ?? '', hostname: p.hostname ?? '', redundancy: p.redundancy ?? '', peer: p.peer ?? '' }));
    }
    case 'net_2110': {
      const n = await get<{ flows: any[] }>(c, `/projects/${pidOf(c)}/network`);
      const ess: Record<string, string> = { video: 'Vidéo', audio: 'Audio', data: 'Données' };
      return n.flows.map((f) => ({ element: f.element, equipment: f.equipment, tag: f.tag, flow: f.signal, essence: ess[f.essence] ?? f.essence, address: f.address ?? '', udp: f.udp != null ? String(f.udp) : '' }));
    }
    case 'net_dmx': {
      const ix = await get<{ pages: any[] }>(c, `/projects/${pidOf(c)}/drawings`);
      const out: Record<string, string>[] = [];
      for (const pg of ix.pages.filter((p) => p.nature === 'lighting')) {
        const d = await get<{ elements: any[] }>(c, `/projects/${pidOf(c)}/drawings/${pg.id}`);
        for (const e of d.elements.filter((x) => x.type === 'equipment')) {
          const ch = (e.dmx?.mode && e.snapshot.dmx_modes.find((m: [string, number]) => m[0] === e.dmx.mode)?.[1]) || e.snapshot.dmx_channels || '';
          out.push({ fixture: e.label ?? e.snapshot.name, tag: e.snapshot.tag, page: pg.name, console: e.dmx?.console ? String(e.dmx.console) : '', universe: e.dmx?.universe ? String(e.dmx.universe) : '', address: e.dmx?.address ? String(e.dmx.address) : '', channels: String(ch), mode: e.dmx?.mode ?? 'défaut' });
        }
      }
      return out.sort((a, b) => Number(a.universe || 1e9) - Number(b.universe || 1e9) || Number(a.address || 0) - Number(b.address || 0));
    }
  }
  return [];
}

// ----- Vues graphiques -----

const sigColor = (id: string) => { const s = refList('signals').find((x) => x.id === id) as any; return s?.data?.color || fnColor(s?.data?.function) || '#15171A'; };
async function drawingPages(c: Ctx, natures: string[]): Promise<{ id: string; label: string }[]> {
  const ix = await get<{ pages: any[] }>(c, `/projects/${pidOf(c)}/drawings`);
  return ix.pages.filter((p) => natures.includes(p.nature) && p.count > 0).map((p) => ({ id: p.id, label: p.name }));
}
async function graphics(c: Ctx, key: string, pages: string[] | undefined): Promise<{ title: string; r: Rendered | SVGSVGElement; w: number; h: number }[]> {
  const pid = pidOf(c);
  const pick = <T extends { id: string }>(list: T[]) => (pages ? list.filter((x) => pages.includes(x.id)) : list);
  if (key === 'plan_schematic' || key === 'plan_av' || key === 'plan_lighting') {
    const nat = key === 'plan_schematic' ? ['schematic'] : key === 'plan_av' ? ['video', 'audio'] : ['lighting'];
    const out = [];
    for (const p of pick(await drawingPages(c, nat))) {
      const d = await get<any>(c, `/projects/${pid}/drawings/${p.id}`);
      const images: Record<string, string> = {};
      for (const e of d.elements.filter((x: any) => x.type === 'image')) { const u = await dataUrl(`/api/projects/${pid}/plans/files/${e.file}`); if (u) images[e.file] = u; }
      const r = drawingSvg(d, { images, fnColor, sigColor, sigName: (id) => refName('signals', id) });
      out.push({ title: d.name, r, w: r.w, h: r.h });
    }
    return out;
  }
  if (key === 'plan_maps') {
    const ix = await get<{ maps: any[] }>(c, `/projects/${pid}/plans`);
    const out = [];
    for (const m of ix.maps) {
      const d = await get<any>(c, `/projects/${pid}/plans/${m.id}`);
      for (const f of d.floors) {
        if (pages && !pages.includes(`${m.id}:${f.id}`)) continue;
        const bg = f.background ? await dataUrl(`/api/projects/${pid}/plans/files/${f.background}`) : null;
        const r = mapFloorSvg(d, f, { bg, sigName: (id) => refName('signals', id) });
        if (r) out.push({ title: `${m.name} · ${f.name}`, r, w: r.w, h: r.h });
      }
    }
    return out;
  }
  if (key === 'plan_synoptic') {
    const out = [];
    for (const ct of pick(c.view.containers.map((x) => ({ id: x.id, label: x.name })))) {
      const s = await get<any>(c, `/projects/${pid}/synoptic?view=container:${ct.id}`);
      const r = synopticSvg(s, { fnColor });
      if (r) out.push({ title: `Synoptique · ${ct.label}`, r, w: r.w, h: r.h });
    }
    return out;
  }
  if (key === 'plan_racks') {
    // Planche d'élévations enregistrée : rendue par le composant de l'écran, hors de la vue.
    const st = await get<{ data: any }>(c, `/projects/${pid}/settings/rack_sheet`);
    const sheet = st.data; if (!sheet?.pages?.length) return [];
    const { default: SheetPage } = await import('../components/SheetPage.svelte');
    const { pageSize, normalize } = await import('./sheet');
    const { loadCtx, shotsFor } = await import('./sheetctx');
    normalize(sheet);
    const ctx = await loadCtx(c.view);
    const site = await get<{ name: string }>(c, '/site');
    const info = { project: c.view.project.name, client: c.view.project.client ?? 'Projet interne', venue: c.view.project.venue ?? '', site: site.name, date: new Date().toLocaleDateString('fr-FR'), locations: Object.fromEntries(c.view.locations.map((l) => [l.id, { name: l.name, code: l.code }])) };
    const host = document.createElement('div'); host.style.cssText = 'position:absolute;left:-100000px;top:0;width:2000px'; document.body.appendChild(host);
    const out = [];
    try {
      const sel = (sheet.pages as any[]).map((p, i) => ({ p, i })).filter(({ i }) => !pages || pages.includes(String(i)));
      for (const { p, i } of sel) {
        const shots = await shotsFor(sheet, ctx, [p]);
        const comp = mount(SheetPage, { target: host, props: { sheet, page: p, ctx, shots, info, index: i, total: sheet.pages.length } });
        await new Promise((r) => requestAnimationFrame(() => r(null)));
        const svg = host.querySelector('svg') as SVGSVGElement;
        const [w, h] = pageSize(sheet);
        const clone = await withLightTheme(() => standalone(svg, [0, 0, w, h]));
        out.push({ title: `Élévations · feuille ${i + 1}`, r: clone, w, h, own: true });
        unmount(comp); host.innerHTML = '';
      }
    } finally { host.remove(); }
    return out;
  }
  return [];
}

export async function choices(c: Ctx, key: string): Promise<{ id: string; label: string }[] | undefined> {
  const pid = pidOf(c);
  if (key === 'plan_schematic') return drawingPages(c, ['schematic']);
  if (key === 'plan_av') return drawingPages(c, ['video', 'audio']);
  if (key === 'plan_lighting') return drawingPages(c, ['lighting']);
  if (key === 'plan_maps') { const ix = await get<{ maps: any[] }>(c, `/projects/${pid}/plans`); return ix.maps.flatMap((m) => m.floors.map((f: any) => ({ id: `${m.id}:${f.id}`, label: `${m.name} · ${f.name}` }))); }
  if (key === 'plan_synoptic') { const out = []; for (const ct of c.view.containers) { const s = await get<any>(c, `/projects/${pid}/synoptic?view=container:${ct.id}`); if (s.nodes.some((n: any) => n.ports.length)) out.push({ id: ct.id, label: ct.name }); } return out; }
  if (key === 'plan_racks') { const st = await get<{ data: any }>(c, `/projects/${pid}/settings/rack_sheet`); return (st.data?.pages ?? []).map((_: any, i: number) => ({ id: String(i), label: `Feuille ${i + 1}` })); }
  return undefined;
}

/** Disponibilité d'un document : « vide » et « en échec » sont distingués. */
export async function probe(c: Ctx, key: string): Promise<Probe> {
  try {
    if (key === 'cover' || key === 'sheet') return { state: 'ok', message: '' };
    if (key === 'recette') { const r = await get<{ total: number }>(c, `/projects/${pidOf(c)}/recette`); return r.total ? { state: 'ok', message: `${r.total} points` } : { state: 'empty', message: 'Aucun point de recette.' }; }
    const ch = await choices(c, key);
    if (ch) return ch.length ? { state: 'ok', message: `${ch.length} page${ch.length > 1 ? 's' : ''}`, choices: ch } : { state: 'empty', message: 'Rien à dessiner.' };
    const rows = await tableRows(c, key);
    return rows.length ? { state: 'ok', message: `${rows.length} ligne${rows.length > 1 ? 's' : ''}` } : { state: 'empty', message: 'Aucune donnée.' };
  } catch (e) { return { state: 'error', message: `Lecture impossible : ${(e as Error).message}` }; }
}

// ----- Assemblage -----

export interface Built { blob: Blob; pages: number; skipped: string[] }
type Section = { title: string; page: number };

function cover(doc: Doc, v: ProjectView, site: { name: string; logo: string | null }, title: string) {
  const [W, H] = size(doc), m = 20;
  if (site.logo) { try { const p = doc.getImageProperties(site.logo); const lw = Math.min(60, (p.width / p.height) * 22); doc.addImage(site.logo, site.logo.startsWith('data:image/png') ? 'PNG' : 'JPEG', m, m, lw, (lw * p.height) / p.width); } catch { /* logo illisible */ } }
  doc.setFont('helvetica', 'bold'); doc.setFontSize(10); doc.setTextColor(91, 99, 109); doc.text(txt(site.name.toUpperCase()), W - m, m + 6, { align: 'right' });
  doc.setFontSize(30); doc.setTextColor(21, 23, 26); doc.text(doc.splitTextToSize(txt(v.project.name), W - 2 * m), m, 80);
  doc.setFont('helvetica', 'normal'); doc.setFontSize(14); doc.text(txt(title), m, 96);
  const h = projectHeader(v.project as any, '');
  doc.setFontSize(11); doc.setTextColor(91, 99, 109);
  [h.client, h.venue, h.dates, `Édité le ${new Date().toLocaleDateString('fr-FR')}`].filter(Boolean).forEach((l, i) => doc.text(txt(l), m, 108 + i * 6.5));
  doc.setDrawColor(21, 23, 26); doc.setLineWidth(0.6); doc.line(m, 140, W - m, 140);
  doc.setFont('helvetica', 'bold'); doc.setFontSize(12); doc.setTextColor(21, 23, 26); doc.text('Sommaire', m, 152);
  void H;
}

export async function build(view: ProjectView, defs: DocDef[], opts: DocOpt[], dossierTitle: string, onStep: (msg: string) => void, only?: string): Promise<Built> {
  await loadRefs();
  const c: Ctx = { view, cache: new Map() };
  const site = await api.get<{ name: string; logo: string | null }>('/site');
  const tpl: TitleBlock | null = await titleBlock();
  const list = opts.filter((o) => o.include && (!only || o.key === only));
  const withCover = !only && list.some((o) => o.key === 'cover');
  const firstFmt = (o: DocOpt): [PaperKey, boolean] => FIXED[o.key] ?? [(o.paper ?? DEFAULT_FMT[o.key]?.[0] ?? 'A4') as PaperKey, o.landscape ?? DEFAULT_FMT[o.key]?.[1] ?? false];
  let doc: Doc | null = null;
  const sections: Section[] = [];
  const skipped: string[] = [];
  const start = async (paper: PaperKey, landscape: boolean) => { if (!doc) doc = await newDoc(paper, landscape); else addPage(doc, paper, landscape); return doc; };
  // Page au format propre (feuille d'élévations, avec son cartouche).
  const startOwn = async (w: number, h: number): Promise<Doc> => { const o = w > h ? 'landscape' : 'portrait'; if (!doc) { await import('svg2pdf.js'); const { jsPDF } = await import('jspdf'); doc = new jsPDF({ unit: 'mm', format: [w, h], orientation: o }); } else doc.addPage([w, h], o); return doc; };
  const baseVars = async (title: string): Promise<Vars> => ({ title, project: view.project.name, client: view.project.client ?? 'Projet interne', address: view.project.venue ?? '', date: new Date().toLocaleDateString('fr-FR'), site: site.name, logo: site.logo ?? '', version: '', view: title });
  if (withCover) { const d = await start('A4', false); cover(d, view, site, dossierTitle); }
  for (const o of list.filter((x) => x.key !== 'cover')) {
    const def = defs.find((d) => d.key === o.key); if (!def) continue;
    const title = o.title || def.title;
    onStep(`${title}…`);
    const pr = await probe(c, o.key);
    if (pr.state !== 'ok') { skipped.push(`${title} : ${pr.state === 'empty' ? 'vide' : 'en échec'}`); continue; }
    const [paper, landscape] = firstFmt(o);
    if (o.key === 'sheet') {
      const d = await start(paper, landscape); sections.push({ title, page: d.getNumberOfPages() });
      const p = view.project as any, h = projectHeader(p, title);
      const rows: [string, string][] = [['Projet', p.name], ['Client', h.client], ['Lieu', h.venue], ['Type de projet', p.service_type ?? ''], ['Dates', h.dates], ['Statut', STATUS[p.status as keyof typeof STATUS]?.label ?? p.status], ['Description', p.description ?? ''], ['Lieux', view.locations.map((l) => l.name).join(', ')], ['Éléments', view.containers.map((x) => x.name).join(', ')]];
      drawTable(d, h, [{ label: 'Rubrique', w: 40 }, { label: 'Valeur', w: 140 }], rows.map(([a, b]) => [a, b]), paper, landscape);
      continue;
    }
    if (o.key === 'recette') {
      const r = await get<any>(c, `/projects/${view.project.id}/recette`);
      const d = await start(paper, false); sections.push({ title, page: d.getNumberOfPages() });
      drawPV(d, r, view, site.name, site.logo, paper === 'A3' ? 'A3' : 'A4');
      continue;
    }
    if (def.columns.length) {
      const rows = await tableRows(c, o.key);
      const cols = (o.columns.length ? o.columns : def.columns).filter((k) => def.columns.includes(k));
      const d = await start(paper, landscape); sections.push({ title, page: d.getNumberOfPages() });
      const head = projectHeader(view.project as any, title);
      const tc: Col[] = cols.map((k) => ({ label: COL_LABEL[k] ?? k, w: COL_W[k] ?? 20, align: ['quantity', 'reserve', 'udp', 'console', 'universe', 'channels'].includes(k) ? 'right' : 'left' }));
      drawTable(d, head, tc, rows.map((r) => cols.map((k) => r[k] ?? '')), paper, landscape);
      continue;
    }
    const gs = await graphics(c, o.key, o.pages);
    if (!gs.length) { skipped.push(`${title} : vide`); continue; }
    let first = true;
    for (const [i, g] of gs.entries()) {
      const own = (g as any).own as boolean | undefined;
      const [pw, ph] = own ? [g.w, g.h] : [0, 0];
      const d = own ? await startOwn(pw, ph) : await start(paper, landscape);
      if (first) { sections.push({ title, page: d.getNumberOfPages() }); first = false; }
      const el = g.r instanceof SVGSVGElement ? g.r : toSvgEl(g.r as Rendered);
      if (own) { await import('svg2pdf.js'); await d.svg(el, { x: 0, y: 0, width: pw, height: ph }); }
      else await svgPage(d, el, g.w, g.h, tpl, { ...(await baseVars(g.title)), sheet: String(i + 1), sheets: String(gs.length), format: `${paper} ${landscape ? 'paysage' : 'portrait'}` });
    }
  }
  if (!doc) throw new Error('Aucun document à imprimer : tous sont vides ou en échec.');
  const d = doc as Doc;
  // Page de fin, sommaire, pagination continue.
  if (withCover && !only) {
    addPage(d, 'A4', false);
    const [W, H] = size(d);
    d.setFont('helvetica', 'normal'); d.setFontSize(10); d.setTextColor(91, 99, 109);
    d.text(txt(`Fin du dossier · ${view.project.name}`), W / 2, H / 2, { align: 'center' });
    d.text(txt(`${site.name} · ${new Date().toLocaleString('fr-FR')}`), W / 2, H / 2 + 6, { align: 'center' });
    d.setPage(1);
    let y = 162;
    d.setFont('helvetica', 'normal'); d.setFontSize(10.5); d.setTextColor(21, 23, 26);
    for (const s of sections) {
      if (y > 280) break;
      d.text(txt(s.title), 20, y); d.text(String(s.page), 190, y, { align: 'right' });
      d.setDrawColor(210, 214, 220); d.setLineDashPattern([0.5, 1], 0); d.line(22 + d.getTextWidth(txt(s.title)), y, 186 - d.getTextWidth(String(s.page)), y); d.setLineDashPattern([], 0);
      y += 7;
    }
  }
  paginate(d, 1, undefined, view.project.name);
  return { blob: d.output('blob'), pages: d.getNumberOfPages(), skipped };
}
