<script lang="ts">
  // Contrôles de construction en atelier (section 9.7) : une liste à cocher par rack, partagée
  // en direct. On coche une ligne d'un clic, ou plusieurs en glissant sur la colonne des cases.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Menu from '../../components/Menu.svelte';
  import Seg from '../../components/Seg.svelte';
  import Loading from '../../components/Loading.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import { ask } from '../../lib/ask.svelte';
  import { route } from '../../lib/router.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { relative } from '../../lib/format';
  import type { ProjectView } from '../../lib/projects';

  interface Item { key: string; family: string; kind: string; label: string; detail: string; checked: { by: string | null; name: string | null; at: string } | null }
  interface Fam { key: string; label: string; done: number; total: number }
  interface Sheet { container: { id: string; name: string }; items: Item[]; families: Fam[]; progress: { done: number; total: number } }
  interface Sum { id: string; name: string; location_id: string; rackable: boolean; done: number; total: number }

  let { view }: { view: ProjectView } = $props();
  let sums = $state<Sum[] | null>(null);
  // L'élément affiché est dans l'adresse (?element=) : un lien ou un rechargement y ramène.
  let cur = $state<string | null>(route.query.get('element'));
  $effect(() => { const e = route.query.get('element'); if (e && e !== untrack(() => cur)) cur = e; });
  function pick(id: string) {
    cur = id;
    const qs = new URLSearchParams(location.search); qs.set('element', id);
    history.replaceState(null, '', `${location.pathname}?${qs}`);
    route.query = qs;
  }
  let sheet = $state<Sheet | null>(null);
  let filter = $state<'all' | 'todo' | 'done'>('all');
  const writable = $derived(can('projets', 'ecriture'));
  const pid = $derived(view.project.id);
  let cview = $state<'inventory' | 'cabling' | 'equipment'>('inventory');
  // Exports du dossier de construction : un menu, et « Génération » tant que le fichier se prépare.
  let exporting = $state<string | null>(null);
  const EXPORTS: [f: 'pdf' | 'csv' | 'doc', label: string, hint: string][] = [['pdf', 'PDF', 'Document à imprimer'], ['csv', 'Tableur', 'Fichier CSV pour Excel ou LibreOffice'], ['doc', 'Word', 'Document modifiable']];
  async function consExport(f: 'pdf' | 'csv' | 'doc') {
    if (!cur || exporting) return;
    exporting = f;
    try { const { construction } = await import('../../lib/construction'); await construction(view, cur, cview, f); } catch (e) { fail(e); } finally { exporting = null; }
  }

  async function loadSums() {
    try {
      sums = (await api.get<{ containers: Sum[] }>(`/projects/${pid}/checks`)).containers;
      if (!cur || !sums.some((s) => s.id === cur)) cur = sums[0]?.id ?? null;
    } catch (e) { fail(e); }
  }
  async function loadSheet() {
    if (!cur) { sheet = null; return; }
    try { sheet = await api.get<Sheet>(`/containers/${cur}/checks`); } catch (e) { fail(e); }
  }
  // Le projet se recharge à chaque événement d'un autre membre : la liste suit.
  $effect(() => { view; untrack(async () => { if (!drag) { await loadSums(); await loadSheet(); } }); });
  $effect(() => { cur; untrack(loadSheet); });

  function syncSum(s: Sheet) {
    sums = sums?.map((x) => (x.id === s.container.id ? { ...x, done: s.progress.done, total: s.progress.total } : x)) ?? null;
  }
  async function send(keys: string[], checked: boolean) {
    if (!keys.length || !cur) return;
    try { const s = await api.post<Sheet>(`/containers/${cur}/checks`, { keys, checked }); sheet = s; syncSum(s); }
    catch (e) { fail(e); await loadSheet(); }
  }

  // Glisser pour cocher : le premier élément fixe le sens (cocher ou décocher).
  let drag: { on: boolean; keys: Set<string> } | null = null;
  function mark(it: Item, on: boolean) {
    it.checked = on ? { by: null, name: 'vous', at: new Date().toISOString() } : null;
  }
  function down(e: PointerEvent, it: Item) {
    if (!writable || e.button > 0) return;
    e.preventDefault();
    drag = { on: !it.checked, keys: new Set([it.key]) };
    mark(it, drag.on);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up, { once: true });
    window.addEventListener('pointercancel', up, { once: true });
  }
  function move(e: PointerEvent) {
    if (!drag || !sheet) return;
    const k = (document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null)?.closest<HTMLElement>('[data-key]')?.dataset.key;
    if (!k || drag.keys.has(k)) return;
    const it = sheet.items.find((x) => x.key === k);
    if (!it || !!it.checked === drag.on) return;
    drag.keys.add(k);
    mark(it, drag.on);
  }
  function up() {
    window.removeEventListener('pointermove', move);
    const d = drag;
    drag = null;
    if (d) send([...d.keys], d.on);
  }
  function key(e: KeyboardEvent, it: Item) {
    if (!writable || (e.key !== ' ' && e.key !== 'Enter')) return;
    e.preventDefault();
    const on = !it.checked;
    mark(it, on);
    send([it.key], on);
  }
  async function clearAll() {
    if (!sheet || !cur) return;
    const id = cur, n = sheet.progress.done, name = sheet.container.name;
    const ok = await ask({
      title: 'Effacer toutes les coches ?', confirmLabel: 'Tout effacer', danger: true,
      text: `Les ${n} ligne${n > 1 ? 's' : ''} cochée${n > 1 ? 's' : ''} de ${name} repasse${n > 1 ? 'nt' : ''} à faire, pour tout le monde.`,
      run: async () => { const s = await api.del<Sheet>(`/containers/${id}/checks`); if (id === cur) sheet = s; syncSum(s); },
    });
    if (ok) toast(`Coches de ${name} effacées.`);
  }

  const shown = $derived(sheet ? sheet.items.filter((i) => filter === 'all' || (filter === 'done') === !!i.checked) : []);
  const pct = (d: number, t: number) => (t ? Math.round((d / t) * 100) : 0);
  const KIND: Record<string, { label: string; icon: string }> = {
    rack: { label: 'Élément', icon: 'server' }, equipment: { label: 'Équipement', icon: 'cpu' }, panel: { label: 'Panel', icon: 'grid' },
    accessory: { label: 'Accessoire', icon: 'layers' }, adapter: { label: 'Adaptateur', icon: 'zap' },
  };
  // Ce que compte chaque famille de la liste à cocher.
  const conns = (sh: Sheet) => sh.families.find((f) => f.key === 'connexions')?.total ?? 0;
  const FAM: Record<string, { label: string; hint: string; note: (n: number, sh: Sheet) => string }> = {
    materiel: { label: 'Matériel', hint: 'L’élément lui-même, puis chaque ligne de matériel (un modèle et ses exemplaires), panel, accessoire et adaptateur.', note: (n) => `${n} ligne${n > 1 ? 's' : ''} : élément et matériel` },
    cables: { label: 'Lots de câbles', hint: 'Câbles à préparer, regroupés par faisceau ou, à défaut, par signal : un lot se coche quand tous ses câbles sont prêts.', note: (n, sh) => `${n} lot${n > 1 ? 's' : ''} pour ${conns(sh)} câble${conns(sh) > 1 ? 's' : ''}` },
    connexions: { label: 'Connexions', hint: 'Un câble par ligne, à brancher sur cet élément (y compris ceux qui partent vers un autre élément), et les attaches d’adaptateurs.', note: (n) => `${n} câble${n > 1 ? 's' : ''} à brancher` },
  };
  const locName = (id: string) => view.locations.find((l) => l.id === id)?.name ?? '';
  const total = $derived(sums?.reduce((a, s) => ({ d: a.d + s.done, t: a.t + s.total }), { d: 0, t: 0 }) ?? { d: 0, t: 0 });
</script>

{#snippet addEl()}<a class="btn primary" href="/projets/{pid}/inventaire"><Icon name="plus" size="sm" />Ajouter un élément</a>{/snippet}
{#snippet busyTrig()}<span class="spin" aria-hidden="true"></span>Génération{/snippet}

{#if sums === null}<Loading label="Chargement des contrôles" />
{:else if !sums.length}
  <EmptyState icon="check" message="Aucun élément à contrôler" hint="Les contrôles de construction suivent les racks et les meubles de l’inventaire." action={writable ? addEl : undefined} />
{:else}
  <div class="ws">
    <aside class="ws-side card">
      <div class="ws-all" title="Somme des listes à cocher de tous les éléments : l’élément lui-même, son matériel, ses lots de câbles et ses connexions. Un câble qui relie deux éléments se coche dans chacun des deux."><span class="flabel">Projet · lignes cochées</span><b class="mono">{total.d} / {total.t}</b><span class="muted small">dans {sums.length} élément{sums.length > 1 ? 's' : ''}</span><span class="bar"><i style="width:{pct(total.d, total.t)}%"></i></span></div>
      <nav aria-label="Éléments">
        {#each sums as s (s.id)}
          <button class="ws-c" class:on={s.id === cur} onclick={() => pick(s.id)} aria-current={s.id === cur ? 'true' : undefined}>
            <span class="ring" style="--p:{pct(s.done, s.total)}" class:full={s.total > 0 && s.done === s.total}>{#if s.total > 0 && s.done === s.total}<Icon name="ok" size="sm" />{:else}<span class="mono">{pct(s.done, s.total)}</span>{/if}</span>
            <span class="ws-ct"><b>{s.name}</b><span class="muted small" title="{s.done} ligne{s.done > 1 ? 's' : ''} cochée{s.done > 1 ? 's' : ''} sur {s.total}">{locName(s.location_id)} · {s.done}/{s.total} ligne{s.total > 1 ? 's' : ''}</span></span>
          </button>
        {/each}
      </nav>
    </aside>

    <section class="ws-main">
      {#if sheet}
        {@const p = sheet.progress}
        <div class="card ws-head">
          <div class="ws-big">
            <span class="eyebrow">Contrôles de construction</span>
            <h2>{sheet.container.name}</h2>
            <div class="cons"><select class="input sm" bind:value={cview} aria-label="Vue du dossier de construction"><option value="inventory">Inventaire</option><option value="cabling">Câblage</option><option value="equipment">Par équipement</option></select>
              <Menu label="Exporter" icon="download" btnClass="btn sm" align="left" menuLabel="Exporter le dossier de construction" disabled={!!exporting} trigger={exporting ? busyTrig : undefined}>
                {#snippet children()}{#each EXPORTS as [f, l, h]}<button role="menuitem" class="exp-it" onclick={() => consExport(f)}><span><b>{l}</b><small>{h}</small></span></button>{/each}{/snippet}
              </Menu>
            </div>
            <div class="ws-num" title="Lignes cochées sur les lignes à contrôler de cet élément"><b class="mono">{p.done}</b><span class="mono">/ {p.total}</span><span class="muted small">lignes cochées</span><span class="chip {p.total && p.done === p.total ? 'ok' : ''}">{pct(p.done, p.total)} %</span></div>
            <span class="bar big"><i style="width:{pct(p.done, p.total)}%"></i></span>
          </div>
          <div class="ws-fams">
            {#each sheet.families as f (f.key)}
              <div class="fam" class:empty={!f.total} title={FAM[f.key]?.hint}>
                <span class="flabel">{FAM[f.key]?.label ?? f.label}</span>
                <b class="mono">{f.done} / {f.total}</b>
                {#if f.total}<span class="bar"><i style="width:{pct(f.done, f.total)}%"></i></span><span class="muted small">{FAM[f.key]?.note(f.total, sheet) ?? ''}</span>{:else}<span class="muted small">Avec le câblage (synoptique)</span>{/if}
              </div>
            {/each}
          </div>
        </div>

        <div class="toolbar ws-tools">
          <Seg label="Lignes affichées" bind:value={filter} options={[{ value: 'all', label: 'Tous', count: p.total }, { value: 'todo', label: 'À faire', count: p.total - p.done }, { value: 'done', label: 'Fait', count: p.done }]} />
          <span class="grow"></span>
          {#if writable}<span class="muted small hide-m">Cliquez une case, ou glissez sur plusieurs</span><button class="btn sm ghost danger" disabled={!p.done} onclick={clearAll}><Icon name="undo" size="sm" />Tout effacer</button>{/if}
        </div>

        <ul class="ws-list card" class:ro={!writable}>
          {#each shown as it (it.key)}
            <li class="ws-row" class:done={!!it.checked} data-key={it.key}>
              <span class="ws-box" role="checkbox" aria-checked={!!it.checked} aria-label={it.label} tabindex={writable ? 0 : -1}
                onpointerdown={(e) => down(e, it)} onkeydown={(e) => key(e, it)}>{#if it.checked}<Icon name="ok" size="sm" />{/if}</span>
              <span class="ws-ico"><Icon name={KIND[it.kind]?.icon ?? 'box'} size="sm" /></span>
              <span class="ws-t"><b>{it.label}</b><span class="muted small">{KIND[it.kind]?.label} · {it.detail}</span></span>
              <span class="ws-who muted small">{#if it.checked}{it.checked.name ?? 'Inconnu'} · {relative(it.checked.at)}{/if}</span>
            </li>
          {:else}<li><EmptyState card={false} icon={filter === 'done' ? 'box' : 'check'} message={filter === 'done' ? 'Rien n’est encore coché' : 'Tout est coché'} /></li>{/each}
        </ul>
      {:else}<Loading label="Chargement de la liste" />{/if}
    </section>
  </div>
{/if}

<style>
  .cons { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; margin: 6px 0; }
  .cons select { width: auto; }
  .exp-it span { display: flex; flex-direction: column; gap: 1px; }
  .exp-it b { font-size: 13.5px; font-weight: 600; }
  .exp-it small { font-size: 12px; color: var(--fg-3); }
  .grow { flex: 1; }
  .ws { display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: 18px; align-items: start; }
  .ws-side { padding: 14px; display: grid; gap: 12px; position: sticky; top: 12px; }
  .ws-all { display: grid; gap: 4px; padding: 4px 6px 10px; border-bottom: 1px solid var(--line); }
  .ws-all b { font-size: 20px; }
  .ws-side nav { display: grid; gap: 4px; }
  .ws-c { display: flex; gap: 10px; align-items: center; padding: 8px; border: 0; border-radius: 12px; background: none; cursor: pointer; text-align: left; font: inherit; color: inherit; }
  .ws-c:hover { background: var(--surface-2); }
  .ws-c:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  /* Sélection habituelle : fond teinté et liseré à gauche. */
  .ws-c.on { background: color-mix(in srgb, var(--accent) 12%, transparent); box-shadow: inset 3px 0 0 var(--accent); color: var(--fg); }
  .ws-c.on .ws-ct b { font-weight: 700; }
  .ws-ct { display: flex; flex-direction: column; min-width: 0; }
  .ws-ct b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ring { --p: 0; width: 36px; height: 36px; flex: none; border-radius: 50%; display: grid; place-items: center; font-size: 11px;
    background: radial-gradient(closest-side, var(--surface) 72%, transparent 74%), conic-gradient(var(--ok) calc(var(--p) * 1%), color-mix(in srgb, var(--fg) 12%, transparent) 0); color: var(--fg); }
  .ring.full { background: var(--ok); color: var(--primary-fg); }
  .ws-main { display: grid; gap: 14px; min-width: 0; }
  .ws-head { padding: 20px 22px; display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr); gap: 24px; align-items: end; }
  .ws-big h2 { font-size: 20px; margin: 4px 0 6px; }
  .ws-num { display: flex; gap: 8px; align-items: baseline; margin-bottom: 8px; }
  .ws-num .small { align-self: center; margin-left: 4px; }
  .ws-num b { font-size: 40px; line-height: 1; letter-spacing: -.02em; }
  .ws-num > .mono:not(b) { font-size: 20px; color: var(--fg-3); }
  .ws-fams { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
  .fam { display: grid; gap: 4px; padding: 12px; border-radius: 14px; background: var(--surface-2); }
  .fam.empty { opacity: .7; color: var(--fg-3); }
  .fam b { font-size: 18px; }
  .bar { display: block; height: 6px; border-radius: 99px; background: color-mix(in srgb, var(--fg) 10%, transparent); overflow: hidden; }
  .bar.big { height: 10px; }
  .bar i { display: block; height: 100%; background: var(--ok); border-radius: inherit; transition: width .3s; }
  .ws-tools { align-items: center; }
  .ws-list { list-style: none; margin: 0; padding: 6px; display: grid; }
  .ws-row { display: grid; grid-template-columns: auto auto minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 10px 12px; border-radius: 12px; }
  .ws-row + .ws-row { border-top: 1px solid var(--line); border-radius: 0; }
  .ws-row.done .ws-t b { color: var(--fg-3); text-decoration: line-through; text-decoration-thickness: 1.5px; }
  .ws-box { width: 30px; height: 30px; border-radius: 9px; border: 2px solid color-mix(in srgb, var(--fg) 30%, transparent); display: grid; place-items: center; cursor: pointer; touch-action: none; user-select: none; transition: background .12s, border-color .12s; }
  .ws-box:hover { border-color: var(--fg); }
  .ws-box:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .ws-row.done .ws-box { background: var(--ok); border-color: var(--ok); color: var(--primary-fg); }
  .ws-list.ro .ws-box { cursor: default; }
  .ws-ico { width: 32px; height: 32px; border-radius: 10px; display: grid; place-items: center; background: var(--surface-2); color: var(--fg-2); }
  .ws-t { display: flex; flex-direction: column; min-width: 0; }
  .ws-t .small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ws-who { white-space: nowrap; }
  @media (max-width: 980px) { .ws { grid-template-columns: 1fr; } .ws-side { position: static; } .ws-side nav { grid-auto-flow: column; grid-auto-columns: minmax(200px, 1fr); overflow-x: auto; } .ws-head { grid-template-columns: 1fr; } }
  @media (max-width: 560px) { .ws-fams { grid-template-columns: 1fr; } .ws-row { grid-template-columns: auto minmax(0, 1fr); } .ws-ico, .ws-who { display: none; } }
</style>
