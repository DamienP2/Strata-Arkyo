<script lang="ts">
  // Sélecteur du mobilier d'un projet (pupitres, consoles, meubles), rangé par emplacement :
  // même présentation que le sélecteur de racks (vignette, nom, sous-titre ; liste avec
  // remplissage). Le remplissage compte les casquettes équipées.
  import Icon from './Icon.svelte';
  import { CONTAINER_KINDS, type Container, type ProjectView } from '../lib/projects';
  import { nf, pl } from '../lib/format';
  import { outside } from '../lib/outside';

  let { view, value, onpick }: { view: ProjectView; value: string; onpick: (v: string) => void } = $props();
  let open = $state(false);
  let btn = $state<HTMLButtonElement>();
  const desks = $derived(view.containers.filter((c) => c.kind === 'desk' || c.kind === 'furniture'));
  const groups = $derived(view.locations.map((l) => ({ l, list: desks.filter((c) => c.location_id === l.id) })).filter((g) => g.list.length));
  const cur = $derived(desks.find((c) => c.id === value) ?? null);
  const casqOf = (d: Container) => view.containers.filter((c) => c.kind === 'casquette' && c.parent_id === d.id);
  /** Casquette équipée : au moins un U posé. */
  const equipped = (d: Container) => casqOf(d).filter((c) => (c.totals.placed_u ?? 0) > 0).length;
  const sub = $derived(cur ? `${view.locations.find((l) => l.id === cur.location_id)?.name ?? ''} · ${CONTAINER_KINDS[cur.kind].label} · ${nf(cur.length_mm ?? 0)} × ${nf(cur.width_mm ?? 0)} mm` : '');
  const pct = (d: Container) => { const n = casqOf(d).length; return n ? (equipped(d) / n) * 100 : 0; };
  function pick(v: string) { open = false; onpick(v); }
  /** Échap ferme le sélecteur seulement (pas la sélection de l'écran derrière) et rend le focus au bouton. */
  function key(e: KeyboardEvent) {
    if (e.key !== 'Escape' || !open) return;
    e.preventDefault(); e.stopPropagation();
    open = false; btn?.focus();
  }
</script>

<div class="rp" use:outside={() => { if (open) open = false; }} onkeydown={key} role="presentation">
  <button class="rp-btn" bind:this={btn} aria-haspopup="listbox" aria-expanded={open} onclick={() => (open = !open)}>
    <span class="rp-ic"><Icon name="layers" size="sm" /></span>
    <span class="rp-t"><b>{cur?.name ?? 'Mobilier'}</b><small>{sub}</small></span>
    <Icon name="chevd" size="sm" />
  </button>
  {#if open}
    <div class="rp-pop" role="listbox" aria-label="Mobilier">
      {#each groups as g (g.l.id)}
        <div class="rp-g">
          <div class="rp-gh"><span class="code mono">{g.l.code || '—'}</span><span class="eyebrow">{g.l.name}</span></div>
          {#each g.list as d (d.id)}
            {@const n = casqOf(d).length}
            <button role="option" aria-selected={value === d.id} class="rp-it" class:on={value === d.id} onclick={() => pick(d.id)} title="{equipped(d)} casquette{pl(equipped(d))} équipée{pl(equipped(d))} sur {n}">
              <span class="rp-n"><b>{d.name}</b><small>{CONTAINER_KINDS[d.kind].label} · {nf(d.length_mm ?? 0)} × {nf(d.width_mm ?? 0)} mm</small></span>
              <span class="rp-fill"><i style="width:{pct(d)}%"></i></span>
              <span class="rp-u mono">{n} casq.</span>
            </button>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* Même dessin que RackPicker.svelte. */
  .rp { position: relative; }
  .rp-btn { display: flex; align-items: center; gap: 10px; min-width: 280px; max-width: 420px; height: 40px; padding: 0 12px 0 5px; border: 0; border-radius: var(--btn-r, 8px); background: var(--btn-bg, var(--surface)); box-shadow: inset 0 0 0 1px var(--btn-edge, var(--line-2)); color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
  .rp-btn:hover { background: var(--btn-bg-h, var(--surface-2)); }
  .rp-btn:focus-visible, .rp-it:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .rp-ic { width: 30px; height: 30px; display: grid; place-items: center; border-radius: 8px; background: var(--surface-3); color: var(--fg-2); flex: none; }
  .rp-t { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.15; }
  .rp-t b { font-size: 14px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rp-t small { font-size: 12px; color: var(--fg-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rp-pop { position: absolute; left: 0; top: calc(100% + 6px); z-index: 50; width: 420px; max-width: calc(100vw - 32px); max-height: min(70vh, 560px); overflow: auto; padding: 6px; border-radius: 12px; background: var(--surface); box-shadow: 0 0 0 1px var(--line-2), 0 24px 48px -16px rgba(0, 0, 0, .55); }
  .rp-it { display: flex; align-items: center; gap: 10px; width: 100%; padding: 8px 10px; border: 0; border-radius: 8px; background: none; color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
  .rp-it:hover { background: var(--surface-2); }
  .rp-it.on { background: color-mix(in srgb, var(--accent) 16%, transparent); box-shadow: inset 2px 0 0 var(--accent); }
  .rp-n small { font-size: 12px; color: var(--fg-3); }
  .rp-g { padding-top: 6px; }
  .rp-g + .rp-g { margin-top: 6px; border-top: 1px solid var(--line); }
  .rp-gh { display: flex; align-items: center; gap: 8px; padding: 4px 10px 6px; }
  .rp-gh .eyebrow { margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rp-gh .code { font-size: 11px; padding: 2px 6px; border-radius: 5px; background: var(--surface-3); color: var(--fg-2); }
  .rp-n { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.2; }
  .rp-n b { font-size: 14px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rp-fill { flex: none; width: 70px; height: 6px; border-radius: 3px; background: var(--surface-3); overflow: hidden; }
  .rp-fill i { display: block; height: 100%; background: var(--ok); }
  .rp-u { flex: none; width: 74px; text-align: right; font-size: 12px; color: var(--fg-2); }
</style>
