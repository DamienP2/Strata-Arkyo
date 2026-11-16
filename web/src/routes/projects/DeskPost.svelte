<script lang="ts">
  // Détail d'un poste du mobilier (panneau de droite) : rôle, nom de l'opérateur et matériel
  // affecté. Un exemplaire n'appartient qu'à un poste ; chaque changement s'enregistre aussitôt
  // (mêmes règles que les postes de l'inventaire).
  import Icon from '../../components/Icon.svelte';
  import { toast } from '../../lib/toast.svelte';
  import type { DeskPosition } from '../../lib/projects';

  interface Gear { id: string; tag: string; label: string }
  let { post, index, positions, gear, writable, onsave, onclose, ondelete }: {
    post: DeskPosition; index: number; positions: DeskPosition[]; gear: Gear[]; writable: boolean;
    onsave: (p: DeskPosition[]) => Promise<boolean>; onclose: () => void; ondelete: () => void;
  } = $props();
  const owner = $derived(new Map(positions.flatMap((p) => p.instance_ids.map((i) => [i, p.id] as const))));
  const free = $derived(gear.filter((g) => !owner.has(g.id)));
  const gearOf = (id: string) => gear.find((g) => g.id === id);
  const set = (change: Partial<DeskPosition>) => onsave(positions.map((x) => (x.id === post.id ? { ...x, ...change } : x)));
  function assign(id: string) { if (id) set({ instance_ids: [...post.instance_ids, id] }); }
  /** Retire un exemplaire du poste : notification avec Annuler (réaffecté au même poste). */
  async function unassign(id: string) {
    const p = post;
    if (!(await set({ instance_ids: p.instance_ids.filter((i) => i !== id) }))) return;
    const g = gearOf(id);
    toast(`« ${g?.tag ?? g?.label ?? 'Exemplaire'} » retiré du poste ${p.role || ''}`.trimEnd() + ' : il est à placer.', 'ok', { run: async () => {
      if (owner.has(id)) return false;
      return (await onsave(positions.map((x) => (x.id === p.id ? { ...x, instance_ids: [...x.instance_ids, id] } : x)))) ? undefined : false;
    } });
  }
</script>

<section>
  <h2 class="sel-h"><span>Poste {index + 1} · {post.role || 'Poste'}</span><button class="icon-btn sm" aria-label="Désélectionner (Échap)" title="Désélectionner (Échap)" onclick={onclose}><Icon name="x" /></button></h2>
  <div class="field"><label for="po-role">Rôle</label><input class="input" id="po-role" value={post.role} disabled={!writable} maxlength="80" placeholder="Réalisateur, scripte, truquiste" onchange={(e) => set({ role: (e.target as HTMLInputElement).value.trim() })}></div>
  <div class="field"><label for="po-name">Opérateur</label><input class="input" id="po-name" value={post.name} disabled={!writable} maxlength="80" placeholder="Nom de la personne (facultatif)" onchange={(e) => set({ name: (e.target as HTMLInputElement).value.trim() })}></div>
  <span class="lbl-t">Matériel du poste · {post.instance_ids.length}</span>
  <ul class="po-list">
    {#each post.instance_ids as id (id)}
      {@const g = gearOf(id)}
      <li><span class="mono small">{g?.tag ?? '—'}</span><span class="po-l">{g?.label ?? 'Exemplaire retiré du mobilier'}</span>{#if writable}<button class="icon-btn sm" aria-label="Retirer {g?.tag ?? 'cet exemplaire'} du poste" title="Retirer du poste" onclick={() => unassign(id)}><Icon name="x" size="sm" /></button>{/if}</li>
    {:else}<li class="muted small">Rien d’affecté : glissez du matériel depuis « À placer », ou choisissez-le ci-dessous.</li>{/each}
  </ul>
  {#if writable && free.length}<select class="input sm" aria-label="Affecter du matériel au poste" onchange={(e) => { const s = e.target as HTMLSelectElement; assign(s.value); s.value = ''; }}><option value="">+ Affecter du matériel</option>{#each free as g}<option value={g.id}>{g.tag} · {g.label}</option>{/each}</select>{/if}
  {#if writable}<button class="btn sm ghost danger" style="align-self:flex-start" title="Supprimer le poste (Suppr) : son matériel redevient à placer" onclick={ondelete}><Icon name="trash" size="sm" />Supprimer le poste</button>{/if}
</section>

<style>
  .sel-h { justify-content: space-between; align-items: center; }
  .sel-h span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .po-list { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; min-width: 0; }
  .po-list li { display: flex; gap: 8px; align-items: center; min-height: 30px; padding: 0 4px 0 10px; border-radius: 8px; background: var(--surface-2); font-size: 13.5px; min-width: 0; }
  .po-list li.muted { background: none; padding: 0; }
  .po-list .mono { flex: none; }
  .po-l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  select { width: 100%; }
</style>
