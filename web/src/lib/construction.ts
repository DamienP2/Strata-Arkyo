// Dossier de construction d'un élément (section 9.8) : vues Inventaire, Câblage ou Par
// équipement ; PDF, tableur (CSV) ou Word. Baie, matériel groupé avec quantités, câbles
// groupés (de, vers, signal, longueur estimée), câblage par équipement (direct ou via adaptateur).

import { api } from './api';
import { drawTable, fileStem, newDoc, paginate, projectHeader, type Col } from './pdfkit';
import type { ProjectView } from './projects';

export type CView = 'inventory' | 'cabling' | 'equipment';
export type CFormat = 'pdf' | 'csv' | 'doc';
interface Section { title: string; cols: Col[]; rows: string[][] }

async function sections(view: ProjectView, cid: string, kind: CView): Promise<{ title: string; sections: Section[] }> {
  const c = view.containers.find((x) => x.id === cid)!;
  const loc = view.locations.find((l) => l.id === c.location_id);
  const out: Section[] = [];
  const baie: string[][] = [['Élément', c.name], ['Lieu', loc?.name ?? ''], ['Nature', c.kind], ...(c.rackable ? [['Hauteur', `${c.height_u ?? '?'} U`], ['Profondeur', c.depth_mm ? `${c.depth_mm} mm` : '']] : [])];
  out.push({ title: 'Baie', cols: [{ label: 'Rubrique', w: 40 }, { label: 'Valeur', w: 140 }], rows: baie });
  const t = await api.get<{ rows: any[] }>(`/projects/${view.project.id}/cables/table`);
  const mine = t.rows.filter((r) => r.a.container_id === cid || r.b.container_id === cid);
  const end = (e: any) => `${e.container_id === cid ? '' : `${e.element} › `}${e.equipment} ${e.tag} › ${e.port}`;
  if (kind === 'inventory') {
    let placed: Record<string, string> = {};
    if (c.rackable) { try { const r = await api.get<{ elements: any[] }>(`/containers/${cid}/rack`); for (const e of r.elements) if (e.placement?.u_start) placed[`${e.label} ${e.tag ?? ''}`.trim()] = `U${e.placement.u_start}${e.placement.face === 'rear' ? ' arr.' : ''}`; } catch { placed = {}; } }
    const arts = view.articles.filter((a) => a.container_id === cid);
    out.push({ title: 'Matériel', cols: [{ label: 'Désignation', w: 70 }, { label: 'Qté', w: 12, align: 'right' }, { label: 'Repères', w: 60 }, { label: 'Position', w: 30 }], rows: arts.map((a) => { const tags = view.instances.filter((i) => i.article_id === a.id).map((i) => i.tag); return [a.label, String(a.quantity), tags.join(' '), tags.map((tg) => placed[`${a.label} ${tg}`]).filter(Boolean).join(', ') || placed[a.label] || '']; }) });
    const groups = new Map<string, { n: number; len: number }>();
    for (const r of mine) { const g = groups.get(r.signal || 'Non défini') ?? { n: 0, len: 0 }; g.n++; g.len += r.length_m ?? r.estimate_m ?? 0; groups.set(r.signal || 'Non défini', g); }
    out.push({ title: 'Câbles (par signal)', cols: [{ label: 'Signal', w: 60 }, { label: 'Nombre', w: 20, align: 'right' }, { label: 'Longueur totale (m)', w: 40, align: 'right' }], rows: [...groups].map(([s, g]) => [s, String(g.n), g.len ? g.len.toFixed(1).replace('.', ',') : '']) });
  } else if (kind === 'cabling') {
    const sig = new Map<string, any[]>(); for (const r of mine) sig.set(r.signal || 'Non défini', [...(sig.get(r.signal || 'Non défini') ?? []), r]);
    for (const [s, rs] of sig) out.push({ title: s === 'Non défini' ? 'Câbles (signal non défini)' : `Câbles ${s}`, cols: [{ label: 'N°', w: 22 }, { label: 'De', w: 70 }, { label: 'Vers', w: 70 }, { label: 'Signal', w: 22 }, { label: 'Long. (m)', w: 18, align: 'right' }], rows: rs.map((r) => [r.number ?? '—', end(r.a), end(r.b), r.signal || 'Non défini', String(r.length_m ?? r.estimate_m ?? '')]) });
  } else {
    const insts = view.instances.filter((i) => view.articles.find((a) => a.id === i.article_id)?.container_id === cid);
    for (const i of insts) {
      const a = view.articles.find((x) => x.id === i.article_id)!;
      const rs = t.rows.filter((r) => r.a.port_ref.startsWith(`i:${i.id}:`) || r.b.port_ref.startsWith(`i:${i.id}:`));
      if (!rs.length) continue;
      out.push({ title: `${a.label} ${i.tag}`, cols: [{ label: 'Port', w: 40 }, { label: 'Câble', w: 22 }, { label: 'Vers', w: 90 }, { label: 'Liaison', w: 28 }], rows: rs.map((r) => { const me = r.a.port_ref.startsWith(`i:${i.id}:`) ? r.a : r.b, other = me === r.a ? r.b : r.a; return [me.port, r.number ?? '—', end(other), other.port_ref.startsWith('a:') ? 'via adaptateur' : 'direct']; }) });
    }
    if (out.length === 1) out.push({ title: 'Câblage', cols: [{ label: 'Information', w: 100 }], rows: [['Aucun équipement câblé dans cet élément.']] });
  }
  return { title: `Dossier de construction · ${c.name}`, sections: out };
}

export async function construction(view: ProjectView, cid: string, kind: CView, format: CFormat) {
  const d = await sections(view, cid, kind);
  const label = { inventory: 'Inventaire', cabling: 'Câblage', equipment: 'Par équipement' }[kind];
  const name = view.containers.find((x) => x.id === cid)!.name;
  const ascii = (v: string) => v.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9-]+/g, '-').replace(/^-|-$/g, '');
  const stem = fileStem(`Construction_${ascii(name)}_${ascii(label)}`, view.project.name);
  let blob: Blob;
  if (format === 'pdf') {
    const doc = await newDoc('A4', false);
    // Sections enchaînées sur la même page ; saut de page seulement quand la place manque.
    const head = projectHeader(view.project, `${d.title} · ${label}`);
    let y: number | undefined;
    for (const s of d.sections) y = drawTable(doc, head, s.cols, s.rows, 'A4', false, { section: s.title, y }) + 5;
    paginate(doc, 1, undefined, view.project.name);
    blob = doc.output('blob');
  } else if (format === 'csv') {
    const q = (v: string) => `"${String(v).replace(/"/g, '""')}"`;
    const lines: string[] = [];
    for (const s of d.sections) { lines.push(q(s.title)); lines.push(s.cols.map((c) => q(c.label)).join(';')); for (const r of s.rows) lines.push(r.map(q).join(';')); lines.push(''); }
    blob = new Blob(['﻿' + lines.join('\r\n')], { type: 'text/csv;charset=utf-8' });
  } else {
    const e = (v: string) => String(v).replace(/&/g, '&amp;').replace(/</g, '&lt;');
    const h = projectHeader(view.project, d.title);
    const html = `<html><head><meta charset="utf-8"><style>body{font-family:Arial,sans-serif;font-size:10pt}table{border-collapse:collapse;width:100%;margin-bottom:14pt}th,td{border:1px solid #999;padding:3pt 5pt;text-align:left}th{background:#eee}</style></head><body><h1>${e(d.title)}</h1><p>${e([h.project, h.client, h.venue, h.dates].filter(Boolean).join(' · '))} — ${e(label)}</p>${d.sections.map((s) => `<h2>${e(s.title)}</h2><table><tr>${s.cols.map((c) => `<th>${e(c.label)}</th>`).join('')}</tr>${s.rows.map((r) => `<tr>${r.map((v) => `<td>${e(v)}</td>`).join('')}</tr>`).join('')}</table>`).join('')}</body></html>`;
    blob = new Blob(['﻿' + html], { type: 'application/msword' });
  }
  const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = `${stem}.${format}`; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 3000);
}
