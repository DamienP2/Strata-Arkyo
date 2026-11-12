// Export d'un dessin SVG de l'application (synoptique, plans…) : styles calculés recopiés
// en ligne, palette claire même en thème sombre, rendu PNG ou JPG sur fond blanc, ou PDF
// vectoriel avec cartouche.

import type { Paper } from './sheet';

const PROPS = ['fill', 'fill-opacity', 'stroke', 'stroke-width', 'stroke-dasharray', 'stroke-linecap', 'stroke-linejoin', 'stroke-opacity', 'opacity',
  'font-family', 'font-size', 'font-weight', 'font-style', 'text-decoration', 'letter-spacing', 'text-anchor', 'dominant-baseline', 'display', 'visibility'];

/** Passe l'application en thème clair le temps d'une capture. */
export async function withLightTheme<T>(f: () => Promise<T> | T): Promise<T> {
  const root = document.documentElement;
  const before = root.dataset.theme;
  root.dataset.theme = 'light';
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  try { return await f(); } finally { if (before === undefined) delete root.dataset.theme; else root.dataset.theme = before; }
}

/** Copie autonome d'un SVG : styles calculés en ligne, sans classes ni variables CSS. */
export function standalone(src: SVGSVGElement, viewBox: [number, number, number, number], skip: (el: Element) => boolean = () => false): SVGSVGElement {
  const clone = src.cloneNode(true) as SVGSVGElement;
  const a = [src, ...src.querySelectorAll('*')];
  const b = [clone, ...clone.querySelectorAll('*')];
  const drop: Element[] = [];
  a.forEach((el, i) => {
    const c = b[i] as SVGElement;
    if (skip(el)) { drop.push(c); return; }
    const cs = getComputedStyle(el);
    let style = '';
    for (const p of PROPS) {
      let v = cs.getPropertyValue(p);
      if (!v) continue;
      if (p === 'text-decoration') v = v.split(' ')[0] ?? v;
      if (p === 'font-weight') v = Number(v) >= 600 || v === 'bold' ? 'bold' : 'normal';
      // Polices standard : rendu identique en PDF et en image.
      if (p === 'font-family') v = /mono|courier|menlo|consol/i.test(v) ? 'Courier, monospace' : /serif/i.test(v) && !/sans/i.test(v) ? 'Times, serif' : 'Helvetica, Arial, sans-serif';
      style += `${p}:${v};`;
    }
    c.setAttribute('style', style);
    c.removeAttribute('class');
  });
  for (const d of drop) d.remove();
  clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
  clone.setAttribute('viewBox', viewBox.join(' '));
  clone.setAttribute('width', String(viewBox[2]));
  clone.setAttribute('height', String(viewBox[3]));
  clone.removeAttribute('style');
  return clone;
}

export async function raster(svg: SVGSVGElement, w: number, h: number, type: 'image/png' | 'image/jpeg', maxPx = 8000): Promise<Blob> {
  const scale = Math.min(3, maxPx / Math.max(w, h));
  const pw = Math.round(w * scale), ph = Math.round(h * scale);
  const s = svg.cloneNode(true) as SVGSVGElement;
  s.setAttribute('width', String(pw)); s.setAttribute('height', String(ph));
  const url = URL.createObjectURL(new Blob([new XMLSerializer().serializeToString(s)], { type: 'image/svg+xml' }));
  try {
    const img = new Image();
    await new Promise((ok, ko) => { img.onload = ok; img.onerror = ko; img.src = url; });
    const c = document.createElement('canvas'); c.width = pw; c.height = ph;
    const g = c.getContext('2d')!; g.fillStyle = '#fff'; g.fillRect(0, 0, pw, ph); g.drawImage(img, 0, 0, pw, ph);
    return await new Promise<Blob>((ok, ko) => c.toBlob((b) => (b ? ok(b) : ko(new Error('Image impossible'))), type, 0.92));
  } finally { URL.revokeObjectURL(url); }
}

export interface Block { title: string; project: string; client: string; view: string; revision: string; site: string; date: string; address?: string; author?: string }

/** PDF vectoriel : le dessin ajusté dans la page, cartouche du site (modèle choisi, sinon le
 * premier actif) en bas à droite. */
export async function pdf(svg: SVGSVGElement, w: number, h: number, paper: Paper, landscape: boolean, block: Block, template?: string | null): Promise<Blob> {
  const kit = await import('./pdfkit');
  const doc = await kit.newDoc(paper as import('./pdfkit').PaperKey, landscape);
  const site = await kit.siteInfo();
  const tpl = await kit.titleBlock(template);
  await kit.svgPage(doc, svg, w, h, tpl, {
    title: block.title, project: block.project, client: block.client, view: block.view, version: block.revision,
    site: block.site || site.name, date: block.date, format: `${paper} ${landscape ? 'paysage' : 'portrait'}`, sheet: '1', sheets: '1',
    logo: site.logo ?? '', address: block.address ?? '', author: block.author ?? '',
  });
  return doc.output('blob');
}

export function save(blob: Blob, name: string) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob); a.download = name; document.body.appendChild(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
}
