<script lang="ts">
  // Contrôles de construction en atelier (section 9.7) : une liste à cocher par rack, partagée
  // en direct. On coche une ligne d'un clic, ou plusieurs en glissant sur la colonne des cases.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
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
  let cur = $state<string | null>(null);
  let sheet = $state<Sheet | null>(null);
  let filter = $state<'all' | 'todo' | 'done'>('all');
  let clearing = $state(false);
  const writable = $derived(can('projets', 'ecriture'));
  const pid = $derived(view.project.id);

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
    try { const s = await api.del<Sheet>(`/containers/${cur}/checks`); sheet = s; syncSum(s); clearing = false; toast('Coches effacées.'); } catch (e) { fail(e); }
  }

  const shown = $derived(sheet ? sheet.items.filter((i) => filter === 'all' || (filter === 'done') === !!i.checked) : []);
  const pct = (d: number, t: number) => (t ? Math.round((d / t) * 100) : 0);
  const KIND: Record<string, { label: string; icon: string }> = {
    rack: { label: 'Baie', icon: 'server' }, equipment: { label: 'Équipements', icon: 'cpu' }, panel: { label: 'Panel', icon: 'grid' },
    accessory: { label: 'Accessoire', icon: 'layers' }, adapter: { label: 'Adaptateurs', icon: 'zap' },
  };
  const locName = (id: string) => view.locations.find((l) => l.id === id)?.name ?? '';
  const total = $derived(sums?.reduce((a, s) => ({ d: a.d + s.done, t: a.t + s.total }), { d: 0, t: 0 }) ?? { d: 0, t: 0 });
</script>

{#if sums === null}<span class="spin"></span>
{:else if !sums.length}
  <section class="card" style="padding:28px;display:grid;gap:12px;justify-items:start">
    <p class="lead" style="margin:0">Aucun contenant à préparer pour l’instant.</p>
    <a class="btn primary" href="/projets/{pid}/inventaire"><Icon name="box" size="sm" />Ajouter un contenant</a>
  </section>
{:else}
  <div class="ws">
    <aside class="ws-side card">
      <div class="ws-all"><span class="flabel">Projet</span><b class="mono">{total.d} / {total.t}</b><span class="bar"><i style="width:{pct(total.d, total.t)}%"></i></span></div>
      <nav aria-label="Contenants">
        {#each sums as s (s.id)}
          <button class="ws-c" class:on={s.id === cur} onclick={() => (cur = s.id)} aria-current={s.id === cur ? 'true' : undefined}>
            <span class="ring" style="--p:{pct(s.done, s.total)}" class:full={s.total > 0 && s.done === s.total}>{#if s.total > 0 && s.done === s.total}<Icon name="ok" size="sm" />{:else}<span class="mono">{pct(s.done, s.total)}</span>{/if}</span>
            <span class="ws-ct"><b>{s.name}</b><span class="muted small">{locName(s.location_id)} · {s.done}/{s.total}</span></span>
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
            <div class="ws-num"><b class="mono">{p.done}</b><span class="mono">/ {p.total}</span><span class="chip {p.total && p.done === p.total ? 'ok' : ''}">{pct(p.done, p.total)} %</span></div>
            <span class="bar big"><i style="width:{pct(p.done, p.total)}%"></i></span>
          </div>
          <div class="ws-fams">
            {#each sheet.families as f (f.key)}
              <div class="fam" class:empty={!f.total}>
                <span class="flabel">{f.label}</span>
                <b class="mono">{f.done} / {f.total}</b>
                {#if f.total}<span class="bar"><i style="width:{pct(f.done, f.total)}%"></i></span>{:else}<span class="muted small">Avec le câblage (synoptique)</span>{/if}
              </div>
            {/each}
          </div>
        </div>

        <div class="toolbar ws-tools">
          <div class="seg" role="group" aria-label="Filtre">{#each [['all', 'Tous'], ['todo', 'À faire'], ['done', 'Fait']] as [k, l]}<button class:on={filter === k} onclick={() => (filter = k as typeof filter)}>{l} <span class="muted mono">{k === 'all' ? p.total : k === 'done' ? p.done : p.total - p.done}</span></button>{/each}</div>
          <span style="flex:1"></span>
          {#if writable}<span class="muted small hide-m">Cliquez une case, ou glissez sur plusieurs</span><button class="btn sm ghost danger" disabled={!p.done} onclick={() => (clearing = true)}><Icon name="undo" size="sm" />Tout effacer</button>{/if}
        </div>

        <ul class="ws-list card" class:ro={!writable}>
          {#each shown as it (it.key)}
            <li class="ws-row" class:done={!!it.checked} data-key={it.key}>
              <span class="ws-box" role="checkbox" aria-checked={!!it.checked} aria-label={it.label} tabindex={writable ? 0 : -1}
                onpointerdown={(e) => down(e, it)} onkeydown={(e) => key(e, it)}>{#if it.checked}<Icon name="ok" size="sm" />{/if}</span>
              <span class="ws-ico"><Icon name={KIND[it.kind]?.icon ?? 'box'} size="sm" /></span>
              <span class="ws-t"><b>{it.label}</b><span class="muted small">{KIND[it.kind]?.label} · {it.detail}</span></span>
              <span class="ws-who muted small">{#if it.checked}{it.checked.name ?? '—'} · {relative(it.checked.at)}{/if}</span>
            </li>
          {:else}<li class="ws-empty muted">{filter === 'done' ? 'Rien n’est encore coché.' : 'Tout est coché.'}</li>{/each}
        </ul>
      {:else}<span class="spin"></span>{/if}
    </section>
  </div>
{/if}

{#if clearing && sheet}
  <Confirm title="Effacer toutes les coches ?" confirmLabel="Tout effacer" danger onconfirm={clearAll} onclose={() => (clearing = false)}>
    <p>Les {sheet.progress.done} éléments cochés de {sheet.container.name} repassent à faire, pour tout le monde.</p>
  </Confirm>
{/if}

<style>
  .ws { display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: 18px; align-items: start; }
  .ws-side { padding: 14px; display: grid; gap: 12px; position: sticky; top: 12px; }
  .ws-all { display: grid; gap: 4px; padding: 4px 6px 10px; border-bottom: 1px solid var(--line); }
  .ws-all b { font-size: 20px; }
  .ws-side nav { display: grid; gap: 4px; }
  .ws-c { display: flex; gap: 10px; align-items: center; padding: 8px; border: 0; border-radius: 12px; background: none; cursor: pointer; text-align: left; font: inherit; color: inherit; }
  .ws-c:hover { background: var(--surface-2); }
  .ws-c.on { background: var(--primary-bg); color: var(--primary-fg); }
  .ws-c.on .muted { color: inherit; opacity: .7; }
  .ws-ct { display: flex; flex-direction: column; min-width: 0; }
  .ws-ct b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ring { --p: 0; width: 36px; height: 36px; flex: none; border-radius: 50%; display: grid; place-items: center; font-size: 10.5px;
    background: radial-gradient(closest-side, var(--surface) 72%, transparent 74%), conic-gradient(var(--ok, #22A06B) calc(var(--p) * 1%), color-mix(in srgb, var(--fg) 12%, transparent) 0); color: var(--fg); }
  .ring.full { background: var(--ok, #22A06B); color: #fff; }
  .ws-main { display: grid; gap: 14px; min-width: 0; }
  .ws-head { padding: 20px 22px; display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr); gap: 24px; align-items: end; }
  .ws-big h2 { font-size: 30px; margin: 4px 0 6px; }
  .ws-num { display: flex; gap: 8px; align-items: baseline; margin-bottom: 8px; }
  .ws-num b { font-size: 44px; line-height: 1; letter-spacing: -.02em; }
  .ws-num > .mono:not(b) { font-size: 20px; color: var(--fg-3); }
  .ws-fams { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
  .fam { display: grid; gap: 4px; padding: 12px; border-radius: 14px; background: var(--surface-2); }
  .fam.empty { opacity: .7; }
  .fam b { font-size: 18px; }
  .bar { display: block; height: 6px; border-radius: 99px; background: color-mix(in srgb, var(--fg) 10%, transparent); overflow: hidden; }
  .bar.big { height: 10px; }
  .bar i { display: block; height: 100%; background: var(--ok, #22A06B); border-radius: inherit; transition: width .3s; }
  .ws-tools { align-items: center; }
  .ws-list { list-style: none; margin: 0; padding: 6px; display: grid; }
  .ws-row { display: grid; grid-template-columns: auto auto minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 10px 12px; border-radius: 12px; }
  .ws-row + .ws-row { border-top: 1px solid var(--line); border-radius: 0; }
  .ws-row.done .ws-t b { color: var(--fg-3); text-decoration: line-through; text-decoration-thickness: 1.5px; }
  .ws-box { width: 30px; height: 30px; border-radius: 9px; border: 2px solid color-mix(in srgb, var(--fg) 30%, transparent); display: grid; place-items: center; cursor: pointer; touch-action: none; user-select: none; transition: background .12s, border-color .12s; }
  .ws-box:hover { border-color: var(--fg); }
  .ws-row.done .ws-box { background: var(--ok, #22A06B); border-color: var(--ok, #22A06B); color: #fff; }
  .ws-list.ro .ws-box { cursor: default; }
  .ws-ico { width: 32px; height: 32px; border-radius: 10px; display: grid; place-items: center; background: var(--surface-2); color: var(--fg-2); }
  .ws-t { display: flex; flex-direction: column; min-width: 0; }
  .ws-t .small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ws-who { white-space: nowrap; }
  .ws-empty { padding: 18px; }
  @media (max-width: 980px) { .ws { grid-template-columns: 1fr; } .ws-side { position: static; } .ws-side nav { grid-auto-flow: column; grid-auto-columns: minmax(200px, 1fr); overflow-x: auto; } .ws-head { grid-template-columns: 1fr; } }
  @media (max-width: 560px) { .ws-fams { grid-template-columns: 1fr; } .ws-row { grid-template-columns: auto minmax(0, 1fr); } .ws-ico, .ws-who { display: none; } }
</style>
