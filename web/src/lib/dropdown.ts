// Listes déroulantes dessinées du design commun Strata (§ 4.14) pour tous les <select> de l'app.
// Le <select> reste la source (valeur, liaisons Svelte, formulaires) et s'affiche fermé comme
// .dd-btn (CSS : apparence retirée, chevron dessiné) ; à l'ouverture, la liste native est
// remplacée par la liste dessinée .dd-list, dans un portail sur <body>, sous le bouton (ou
// au-dessus s'il manque de place). Choisir règle la valeur et envoie input puis change.
// Clavier : ↓ / Entrée / Espace ouvre ; ↑ ↓ (options désactivées sautées), Début, Fin ; Entrée
// choisit ; Échap ferme et rend le focus ; Tab ferme. Pointeur ailleurs, molette hors de la
// liste ou redimensionnement ferment.

const CHECK = '<svg viewBox="0 0 24 24" width="12" height="12" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>';

let cur: { sel: HTMLSelectElement; list: HTMLDivElement; opts: HTMLDivElement[]; hover: number } | null = null;

const drawn = (t: EventTarget | null): t is HTMLSelectElement =>
  t instanceof HTMLSelectElement && !t.multiple && t.size <= 1 && !t.disabled;

function close(focus = false) {
  if (!cur) return;
  const { sel, list } = cur;
  list.remove();
  sel.classList.remove('dd-open');
  cur = null;
  if (focus) sel.focus({ preventScroll: true });
}

function setHover(i: number) {
  if (!cur) return;
  cur.opts.forEach((o, k) => o.classList.toggle('hover', k === i));
  cur.hover = i;
  cur.opts[i]?.scrollIntoView({ block: 'nearest' });
}

function choose(i: number) {
  if (!cur) return;
  const o = cur.opts[i];
  if (!o || o.classList.contains('off')) return;
  const sel = cur.sel, idx = Number(o.dataset.i);
  close(true);
  if (sel.selectedIndex === idx) return;
  sel.selectedIndex = idx;
  sel.dispatchEvent(new Event('input', { bubbles: true }));
  sel.dispatchEvent(new Event('change', { bubbles: true }));
}

function place(sel: HTMLSelectElement, list: HTMLDivElement) {
  const b = sel.getBoundingClientRect(), m = 8, vw = innerWidth, vh = innerHeight;
  list.style.minWidth = `${Math.max(160, b.width)}px`;
  const h = Math.min(list.scrollHeight, 300);
  const below = vh - b.bottom - 4 - m, above = b.top - 4 - m;
  const up = below < 240 && above > below;
  list.classList.toggle('up', up);
  list.style.maxHeight = `${Math.max(120, Math.min(300, up ? above : below))}px`;
  list.style.top = `${up ? Math.max(m, b.top - 4 - Math.min(h, above)) : b.bottom + 4}px`;
  const w = list.offsetWidth;
  list.style.left = `${Math.max(m, Math.min(b.left, vw - w - m))}px`;
}

function open(sel: HTMLSelectElement) {
  close();
  const list = document.createElement('div');
  list.className = 'dd-list';
  list.setAttribute('role', 'listbox');
  const name = sel.getAttribute('aria-label') ?? (sel.id ? document.querySelector(`label[for="${CSS.escape(sel.id)}"]`)?.textContent?.trim() : '') ?? '';
  if (name) list.setAttribute('aria-label', name);
  const opts: HTMLDivElement[] = [];
  let group: HTMLOptGroupElement | null = null;
  Array.from(sel.options).forEach((o, i) => {
    if (o.hidden) return;
    const g = o.parentElement instanceof HTMLOptGroupElement ? o.parentElement : null;
    if (g && g !== group) {
      const t = document.createElement('div');
      t.className = 'dd-group';
      t.textContent = g.label;
      list.appendChild(t);
    }
    group = g;
    const d = document.createElement('div');
    const off = o.disabled || !!g?.disabled;
    d.className = `dd-opt${i === sel.selectedIndex ? ' on' : ''}${off ? ' off' : ''}`;
    d.setAttribute('role', 'option');
    d.setAttribute('aria-selected', String(i === sel.selectedIndex));
    if (off) d.setAttribute('aria-disabled', 'true');
    d.dataset.i = String(i);
    const ck = document.createElement('span');
    ck.className = 'dd-check';
    if (i === sel.selectedIndex) ck.innerHTML = CHECK;
    const tx = document.createElement('span');
    tx.className = 'dd-text';
    tx.textContent = o.label || o.text || '—';
    d.append(ck, tx);
    const k = opts.length;
    d.addEventListener('pointerenter', () => { if (!off) setHover(k); });
    d.addEventListener('click', (e) => { e.preventDefault(); choose(k); });
    opts.push(d);
    list.appendChild(d);
  });
  if (!opts.length) {
    const e = document.createElement('div');
    e.className = 'dd-empty';
    e.textContent = 'Rien à choisir';
    list.appendChild(e);
  }
  list.addEventListener('pointerdown', (e) => e.preventDefault());
  // Une boîte de dialogue ouverte garde la liste au-dessus d'elle.
  document.body.appendChild(list);
  sel.classList.add('dd-open');
  cur = { sel, list, opts, hover: -1 };
  place(sel, list);
  const on = opts.findIndex((o) => o.classList.contains('on'));
  setHover(on >= 0 ? on : opts.findIndex((o) => !o.classList.contains('off')));
}

function step(dir: 1 | -1, from?: number) {
  if (!cur) return;
  const n = cur.opts.length;
  let i = from ?? cur.hover;
  for (let k = 0; k < n; k++) {
    i = i + dir;
    if (i < 0 || i >= n) return;
    if (!cur.opts[i]!.classList.contains('off')) { setHover(i); return; }
  }
}

let installed = false;
export function installDropdowns() {
  if (installed || typeof document === 'undefined') return;
  installed = true;
  // Le bouton : pas de liste native, la liste dessinée s'ouvre ou se ferme.
  const press = (e: Event) => {
    const t = e.target;
    if (!drawn(t)) return;
    if ((e as MouseEvent).button) return;
    e.preventDefault();
    if (e.type === 'mousedown') return;
    t.focus({ preventScroll: true });
    if (cur?.sel === t) close(); else open(t);
  };
  document.addEventListener('pointerdown', press, true);
  document.addEventListener('mousedown', press, true);
  document.addEventListener('click', (e) => { if (drawn(e.target)) e.preventDefault(); }, true);
  document.addEventListener('pointerdown', (e) => {
    if (cur && !cur.list.contains(e.target as Node) && e.target !== cur.sel) close();
  }, true);
  document.addEventListener('keydown', (e) => {
    if (cur) {
      const k = e.key;
      if (k === 'Escape') { e.preventDefault(); e.stopPropagation(); close(true); return; }
      if (k === 'Tab') { close(); return; }
      if (k === 'ArrowDown') { e.preventDefault(); e.stopPropagation(); step(1); return; }
      if (k === 'ArrowUp') { e.preventDefault(); e.stopPropagation(); step(-1); return; }
      if (k === 'Home') { e.preventDefault(); step(1, -1); return; }
      if (k === 'End') { e.preventDefault(); step(-1, cur.opts.length); return; }
      if (k === 'Enter' || k === ' ') { e.preventDefault(); e.stopPropagation(); choose(cur.hover); return; }
      return;
    }
    const t = e.target;
    if (!drawn(t) || e.altKey || e.ctrlKey || e.metaKey) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === ' ') {
      e.preventDefault(); e.stopPropagation(); open(t);
    }
  }, true);
  addEventListener('wheel', (e) => { if (cur && !cur.list.contains(e.target as Node)) close(); }, { capture: true, passive: true });
  addEventListener('resize', () => close());
  addEventListener('blur', () => close());
  // Le <select> retiré du document (changement de page) emporte sa liste.
  new MutationObserver(() => { if (cur && !cur.sel.isConnected) close(); }).observe(document.body, { childList: true, subtree: true });
}
