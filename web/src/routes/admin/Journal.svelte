<script lang="ts">
  // Journal d'administration : qui a fait quoi, sur quel objet et dans quel projet. Libellés
  // français fournis par le serveur, filtres (membre, action, projet, période), un groupe par jour.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Loading from '../../components/Loading.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import { api } from '../../lib/api';
  import { byDay, timeOf } from '../../lib/format';
  import type { Member } from '../../lib/types';

  interface Entry { id: number; at: string; actor_id: string | null; action: string; object_type: string; action_label: string; object_label: string; subject: string; detail: string; project_id: string | null; project_name: string | null }
  interface R { entries: Entry[]; actions: { key: string; label: string }[]; projects: { id: string; name: string }[] }

  // Les actions arrivent par pages : « Afficher plus » en demande PAGE de plus, jusqu'à MAX (borne du serveur).
  const PAGE = 200, MAX = 1000;
  let limit = $state(PAGE);
  let r = $state<R | null>(null);
  let members = $state<Member[]>([]);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let actor = $state('');
  let kind = $state('');
  let project = $state('');
  let period = $state<'' | '1' | '7' | '30'>('');
  const PERIODS: [typeof period, string][] = [['', 'Toute la période'], ['1', 'Aujourd’hui'], ['7', '7 derniers jours'], ['30', '30 derniers jours']];
  const names = $derived(Object.fromEntries(members.map((x) => [x.id, x.name])));
  // Une action de plus que la page est demandée pour savoir s'il en reste.
  const entries = $derived(r ? r.entries.slice(0, limit) : []);
  const more = $derived(!!r && r.entries.length > limit);
  const capped = $derived(!!r && limit >= MAX && r.entries.length >= MAX);
  const days = $derived(byDay(entries));
  const filtered = $derived(!!(actor || kind || project || period));

  let seq = 0;
  async function load() {
    const n = ++seq;
    loading = true;
    const q = new URLSearchParams({ limit: String(Math.min(limit + 1, MAX)) });
    if (actor) q.set('actor', actor);
    if (kind) q.set('action', kind);
    if (project) q.set('project', project);
    if (period) { const d = new Date(); d.setHours(0, 0, 0, 0); d.setDate(d.getDate() - (Number(period) - 1)); q.set('since', d.toISOString()); }
    try { const res = await api.get<R>(`/audit?${q}`); if (n === seq) { r = res; error = null; } }
    catch (e) { if (n === seq) error = (e as Error).message; }
    finally { if (n === seq) loading = false; }
  }
  // Un changement de filtre repart de la première page.
  $effect(() => { actor; kind; project; period; untrack(() => { limit = PAGE; load(); }); });
  function showMore() { limit = Math.min(limit + PAGE, MAX); load(); }
  api.get<{ members: Member[] }>('/members?removed=true').then((m) => (members = m.members)).catch(() => {});
  const reset = () => { actor = ''; kind = ''; project = ''; period = ''; };
</script>

{#if error}<div class="alert err">{error}</div>{:else}
  <div class="toolbar jr-bar">
    <select class="input" bind:value={actor} aria-label="Membre"><option value="">Tous les membres</option>{#each members as m (m.id)}<option value={m.id}>{m.name}</option>{/each}</select>
    <select class="input" bind:value={kind} aria-label="Type d’action"><option value="">Toutes les actions</option>{#each r?.actions ?? [] as a (a.key)}<option value={a.key}>{a.label}</option>{/each}</select>
    <select class="input" bind:value={project} aria-label="Projet"><option value="">Tous les projets</option>{#each r?.projects ?? [] as p (p.id)}<option value={p.id}>{p.name}</option>{/each}</select>
    <select class="input" bind:value={period} aria-label="Période">{#each PERIODS as [k, l]}<option value={k}>{l}</option>{/each}</select>
    {#if filtered}<button class="btn sm ghost" onclick={reset}><Icon name="x" size="sm" />Effacer les filtres</button>{/if}
    <span class="tb-gap"></span>
    {#if loading && r}<Loading inline label="Mise à jour du journal" />{/if}
    {#if r}<span class="muted small">{entries.length} action{entries.length > 1 ? 's' : ''}{more || capped ? ' (les plus récentes)' : ''}</span>{/if}
  </div>
  <section class="card tbl-card">
    {#if r === null}<Loading label="Chargement du journal" />
    {:else}
    <div class="tbl-wrap">
      <table class="tbl stack jr">
        <thead><tr><th>Heure</th><th>Qui</th><th>Action</th><th>Objet</th><th>Projet</th></tr></thead>
        <tbody>
          {#each days as d (d.key)}
            <tr class="grp"><th colspan="5">{d.title}<span class="muted">{' · '}{d.items.length} action{d.items.length > 1 ? 's' : ''}</span></th></tr>
            {#each d.items as e (e.id)}
              <tr>
                <td class="mono jr-t">{timeOf(e.at)}</td>
                <td>{e.actor_id ? (names[e.actor_id] ?? 'Compte supprimé') : 'Système'}</td>
                <td class="jr-w">{e.action_label}</td>
                <td class="jr-w jr-o">{e.object_label}{#if e.subject}<span class="muted">{' · '}</span><b>{e.subject}</b>{/if}{#if e.detail}<span class="muted small jr-d">{e.detail}</span>{/if}</td>
                <td class="jr-w" data-l="Projet">{#if e.project_id && e.project_name !== 'Projet supprimé'}<a class="cell-link" href="/projets/{e.project_id}">{e.project_name}</a>{:else if e.project_name}<span class="muted">{e.project_name}</span>{:else}<span class="muted">—</span>{/if}</td>
              </tr>
            {/each}
          {:else}<tr><td colspan="5" class="first">
            {#if filtered}
              <EmptyState card={false} icon="search" message="Aucune action ne correspond aux filtres.">{#snippet action()}<button class="btn sm" onclick={reset}>Effacer les filtres</button>{/snippet}</EmptyState>
            {:else}<EmptyState card={false} icon="terminal" message="Le journal est vide." hint="Les actions des membres apparaîtront ici, groupées par jour." />{/if}
          </td></tr>{/each}
        </tbody>
      </table>
    </div>
    {#if more}<div class="tbl-more"><button class="btn sm" onclick={showMore} disabled={loading}>{#if loading}<span class="spin" aria-hidden="true"></span>{/if}Afficher plus ({entries.length} affichées)</button></div>
    {:else if capped}<div class="muted small tbl-more">Les {MAX} actions les plus récentes : affinez avec les filtres pour remonter plus loin.</div>{/if}
    {/if}
  </section>
{/if}

<style>
  .jr-bar { margin-bottom: 12px; }
  .jr-bar select { width: auto; }
  .jr-t { white-space: nowrap; }
  .jr-d { display: block; }
  /* Téléphone (.tbl.stack) : heure et auteur sur une ligne, puis action, objet et projet chacun sur toute la largeur. */
  @media (max-width: 860px) {
    .jr td.jr-w { grid-column: 1 / -1; }
    .jr td.jr-o { display: block; }
  }
</style>
