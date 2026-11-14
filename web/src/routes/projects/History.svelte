<script lang="ts">
  import { untrack } from 'svelte';
  import { api } from '../../lib/api';
  import { fail } from '../../lib/toast.svelte';
  import { byDay, timeOf } from '../../lib/format';
  import ProjectPoints from '../../components/ProjectPoints.svelte';
  import type { Member } from '../../lib/types';

  let { projectId }: { projectId: string } = $props();
  // Libellés, nom de l'objet et changement fournis par le serveur (comme le journal d'administration).
  interface Entry { id: number; at: string; actor_id: string | null; action_label: string; object_label: string; subject: string; detail: string }
  let entries = $state<Entry[]>([]);
  let names = $state<Record<string, string>>({});
  Promise.all([api.get<{ entries: Entry[] }>(`/projects/${untrack(() => projectId)}/history?limit=300`), api.get<{ members: Member[] }>('/members?removed=true')])
    .then(([h, m]) => { entries = h.entries; names = Object.fromEntries(m.members.map((x) => [x.id, x.name])); })
    .catch(fail);
  const days = $derived(byDay(entries));
</script>

<ProjectPoints pid={projectId} onrestored={() => location.reload()} />

<section class="card" style="padding:6px 18px 10px">
  <div class="tbl-wrap"><table class="tbl hs">
    <thead><tr><th>Heure</th><th>Qui</th><th>Action</th><th>Objet et changement</th></tr></thead>
    <tbody>
      {#each days as d (d.key)}
        <tr class="day"><th colspan="4">{d.title}<span class="muted">{' · '}{d.items.length} modification{d.items.length > 1 ? 's' : ''}</span></th></tr>
        {#each d.items as e (e.id)}<tr><td class="mono" style="white-space:nowrap">{timeOf(e.at)}</td><td>{e.actor_id ? (names[e.actor_id] ?? 'Compte supprimé') : 'Système'}</td><td>{e.action_label}</td><td>{e.object_label}{#if e.subject}<span class="muted">{' · '}</span><b>{e.subject}</b>{/if}{#if e.detail}<span class="muted small hs-d">{e.detail}</span>{/if}</td></tr>{/each}
      {:else}<tr><td colspan="4" class="empty">Aucune modification enregistrée.</td></tr>{/each}
    </tbody>
  </table></div>
</section>

<style>
  .hs tr.day th { text-align: left; padding: 16px 8px 6px; font: 650 14px var(--font); letter-spacing: 0; text-transform: none; color: var(--fg); border-bottom: 1px solid var(--line); }
  .hs-d { display: block; }
</style>
