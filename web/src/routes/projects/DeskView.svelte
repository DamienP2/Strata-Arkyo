<script lang="ts">
  // Pupitre (ou poste opérateur) : chaque poste reçoit le matériel de l'élément qui lui
  // revient. Un exemplaire n'appartient qu'à un poste ; chaque affectation s'enregistre aussitôt.
  import Icon from '../../components/Icon.svelte';
  import type { DeskPosition } from '../../lib/projects';

  interface Gear { id: string; tag: string; label: string }
  let { positions, gear, writable, onsave }: { positions: DeskPosition[]; gear: Gear[]; writable: boolean; onsave: (p: DeskPosition[]) => Promise<boolean> } = $props();
  const owner = $derived(new Map(positions.flatMap((p) => p.instance_ids.map((i) => [i, p.id] as const))));
  const free = $derived(gear.filter((g) => !owner.has(g.id)));
  const gearOf = (id: string) => gear.find((g) => g.id === id);
  function assign(p: DeskPosition, id: string) {
    if (!id) return;
    onsave(positions.map((x) => (x.id === p.id ? { ...x, instance_ids: [...x.instance_ids, id] } : x)));
  }
  function unassign(p: DeskPosition, id: string) {
    onsave(positions.map((x) => (x.id === p.id ? { ...x, instance_ids: x.instance_ids.filter((i) => i !== id) } : x)));
  }
</script>

<div class="dv">
  <div class="dv-desk">
    {#each positions as p, i (p.id)}
      <section class="dv-post">
        <div class="dv-h"><span class="dv-n mono">{i + 1}</span><span><b>{p.role || 'Poste'}</b>{#if p.name}<span class="muted small">{' · '}{p.name}</span>{/if}</span></div>
        <ul>
          {#each p.instance_ids as id (id)}
            {@const g = gearOf(id)}
            <li><span class="mono small">{g?.tag ?? '—'}</span><span class="dv-l">{g?.label ?? 'Exemplaire retiré de l’élément'}</span>{#if writable}<button class="icon-btn sm" aria-label="Retirer du poste" onclick={() => unassign(p, id)}><Icon name="x" size="sm" /></button>{/if}</li>
          {:else}<li class="muted small">Rien d’affecté</li>{/each}
        </ul>
        {#if writable && free.length}<select class="input" aria-label="Affecter du matériel au poste {p.role}" onchange={(e) => { const s = e.target as HTMLSelectElement; assign(p, s.value); s.value = ''; }}><option value="">+ Affecter…</option>{#each free as g}<option value={g.id}>{g.tag} · {g.label}</option>{/each}</select>{/if}
      </section>
    {/each}
  </div>
  <p class="muted small" style="margin:8px 0 0">{gear.length - free.length} / {gear.length} exemplaire{gear.length > 1 ? 's' : ''} affecté{gear.length - free.length > 1 ? 's' : ''} · les postes se modifient avec « Modifier ».</p>
</div>

<style>
  .dv { margin-top: 14px; }
  .dv-desk { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 10px; padding: 12px; border-radius: 16px 16px 30px 30px; background: linear-gradient(180deg, var(--surface-2), color-mix(in srgb, var(--fg) 8%, var(--surface-2))); border-bottom: 6px solid color-mix(in srgb, var(--fg) 18%, transparent); }
  .dv-post { min-width: 0; padding: 12px; border-radius: 12px; background: var(--surface); display: grid; gap: 8px; align-content: start; box-shadow: 0 1px 0 var(--line); }
  .dv-h { display: flex; gap: 8px; align-items: center; }
  .dv-n { width: 22px; height: 22px; border-radius: 50%; display: grid; place-items: center; font-size: 10.5px; background: var(--fg); color: var(--bg); flex: none; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; min-width: 0; }
  li { display: flex; gap: 8px; align-items: center; font-size: 13.5px; min-width: 0; }
  li .mono { flex: none; }
  .dv-l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  select { height: 36px; font-size: 13.5px; }
</style>
