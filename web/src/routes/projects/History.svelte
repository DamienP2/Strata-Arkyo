<script lang="ts">
  import { untrack } from 'svelte';
  import { api } from '../../lib/api';
  import { fail } from '../../lib/toast.svelte';
  import { byDay, timeOf } from '../../lib/format';
  import ProjectPoints from '../../components/ProjectPoints.svelte';
  import Loading from '../../components/Loading.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import type { Member } from '../../lib/types';

  let { projectId }: { projectId: string } = $props();
  // Libellés, nom de l'objet et changement fournis par le serveur (comme le journal d'administration).
  interface Entry { id: number; at: string; actor_id: string | null; action_label: string; object_label: string; subject: string; detail: string }
  /** Le serveur rend au plus 500 entrées : on en charge 300, puis tout ce qu'il peut sur demande. */
  const MAX = 500;
  let limit = $state(300);
  let entries = $state<Entry[] | null>(null);
  let more = $state(false);
  let names = $state<Record<string, string>>({});
  const pid = untrack(() => projectId);
  async function load() {
    try { entries = (await api.get<{ entries: Entry[] }>(`/projects/${pid}/history?limit=${limit}`)).entries; } catch (e) { fail(e); entries ??= []; }
  }
  api.get<{ members: Member[] }>('/members?removed=true').then((m) => (names = Object.fromEntries(m.members.map((x) => [x.id, x.name]))), fail);
  load();
  async function showMore() { more = true; limit = MAX; await load(); more = false; }
  const days = $derived(byDay(entries ?? []));
</script>

<ProjectPoints pid={projectId} onrestored={() => location.reload()} />

<section class="card tbl-card">
  {#if entries === null}<Loading label="Chargement de l’historique" />
  {:else if !entries.length}<EmptyState card={false} icon="clock" message="Aucune modification enregistrée" hint="Chaque modification du projet apparaîtra ici, avec son auteur et son heure." />
  {:else}
    <div class="tbl-wrap"><table class="tbl hs">
      <thead><tr><th>Heure</th><th>Qui</th><th>Action</th><th>Objet et changement</th></tr></thead>
      <tbody>
        {#each days as d (d.key)}
          <tr class="grp"><th colspan="4">{d.title}<span class="muted">{' · '}{d.items.length} modification{d.items.length > 1 ? 's' : ''}</span></th></tr>
          {#each d.items as e (e.id)}<tr><td class="mono hs-t">{timeOf(e.at)}</td><td>{e.actor_id ? (names[e.actor_id] ?? 'Compte supprimé') : 'Système'}</td><td>{e.action_label}</td><td>{e.object_label}{#if e.subject}<span class="muted">{' · '}</span><b>{e.subject}</b>{/if}{#if e.detail}<span class="muted small hs-d">{e.detail}</span>{/if}</td></tr>{/each}
        {/each}
      </tbody>
    </table></div>
    {#if entries.length >= limit}
      <div class="tbl-more">
        {#if limit < MAX}<button class="btn sm" disabled={more} onclick={showMore}>{#if more}<span class="spin" aria-hidden="true"></span>{/if}Afficher plus ({entries.length} dernières modifications affichées)</button>
        {:else}<span class="muted small">Seules les {MAX} dernières modifications sont affichées.</span>{/if}
      </div>
    {/if}
  {/if}
</section>

<style>
  .hs tr.grp th { text-align: left; }
  .hs-t { white-space: nowrap; }
  .hs-d { display: block; }
</style>
