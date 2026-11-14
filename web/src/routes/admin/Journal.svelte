<script lang="ts">
  // Journal d'administration : qui a fait quoi, sur quel objet et dans quel projet. Libellés
  // français fournis par le serveur, filtres (membre, action, projet, période), un groupe par jour.
  import Icon from '../../components/Icon.svelte';
  import { api } from '../../lib/api';
  import { byDay, timeOf } from '../../lib/format';
  import type { Member } from '../../lib/types';

  interface Entry { id: number; at: string; actor_id: string | null; action: string; object_type: string; action_label: string; object_label: string; subject: string; detail: string; project_id: string | null; project_name: string | null }
  interface R { entries: Entry[]; actions: { key: string; label: string }[]; projects: { id: string; name: string }[] }

  const LIMIT = 500;
  let r = $state<R | null>(null);
  let members = $state<Member[]>([]);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let actor = $state('');
  let action = $state('');
  let project = $state('');
  let period = $state<'' | '1' | '7' | '30'>('');
  const PERIODS: [typeof period, string][] = [['', 'Toute la période'], ['1', 'Aujourd’hui'], ['7', '7 derniers jours'], ['30', '30 derniers jours']];
  const names = $derived(Object.fromEntries(members.map((x) => [x.id, x.name])));
  const days = $derived(byDay(r?.entries ?? []));
  const filtered = $derived(!!(actor || action || project || period));

  async function load() {
    loading = true;
    const q = new URLSearchParams({ limit: String(LIMIT) });
    if (actor) q.set('actor', actor);
    if (action) q.set('action', action);
    if (project) q.set('project', project);
    if (period) { const d = new Date(); d.setHours(0, 0, 0, 0); d.setDate(d.getDate() - (Number(period) - 1)); q.set('since', d.toISOString()); }
    try { r = await api.get<R>(`/audit?${q}`); error = null; } catch (e) { error = (e as Error).message; } finally { loading = false; }
  }
  $effect(() => { actor; action; project; period; load(); });
  api.get<{ members: Member[] }>('/members?removed=true').then((m) => (members = m.members)).catch(() => {});
  const reset = () => { actor = ''; action = ''; project = ''; period = ''; };
</script>

{#if error}<div class="alert err">{error}</div>{:else}
  <div class="toolbar jr-bar">
    <select class="input" bind:value={actor} aria-label="Membre"><option value="">Tous les membres</option>{#each members as m (m.id)}<option value={m.id}>{m.name}</option>{/each}</select>
    <select class="input" bind:value={action} aria-label="Type d’action"><option value="">Toutes les actions</option>{#each r?.actions ?? [] as a (a.key)}<option value={a.key}>{a.label}</option>{/each}</select>
    <select class="input" bind:value={project} aria-label="Projet"><option value="">Tous les projets</option>{#each r?.projects ?? [] as p (p.id)}<option value={p.id}>{p.name}</option>{/each}</select>
    <select class="input" bind:value={period} aria-label="Période">{#each PERIODS as [k, l]}<option value={k}>{l}</option>{/each}</select>
    {#if filtered}<button class="btn sm ghost" onclick={reset}><Icon name="x" size="sm" />Effacer les filtres</button>{/if}
    <span style="flex:1"></span>
    {#if loading}<span class="spin"></span>{/if}
    {#if r}<span class="muted small">{r.entries.length} action{r.entries.length > 1 ? 's' : ''}{r.entries.length >= LIMIT ? ` (les ${LIMIT} plus récentes)` : ''}</span>{/if}
  </div>
  <section class="card" style="padding:6px 18px">
    <div class="tbl-wrap">
      <table class="tbl jr">
        <thead><tr><th>Heure</th><th>Qui</th><th>Action</th><th>Objet</th><th>Projet</th></tr></thead>
        <tbody>
          {#each days as d (d.key)}
            <tr class="day"><th colspan="5">{d.title}<span class="muted">{' · '}{d.items.length} action{d.items.length > 1 ? 's' : ''}</span></th></tr>
            {#each d.items as e (e.id)}
              <tr>
                <td class="mono" style="white-space:nowrap">{timeOf(e.at)}</td>
                <td>{e.actor_id ? (names[e.actor_id] ?? 'Compte supprimé') : 'Système'}</td>
                <td>{e.action_label}</td>
                <td>{e.object_label}{#if e.subject}<span class="muted">{' · '}</span><b>{e.subject}</b>{/if}{#if e.detail}<span class="muted small jr-d">{e.detail}</span>{/if}</td>
                <td>{#if e.project_id && e.project_name !== 'Projet supprimé'}<a href="/projets/{e.project_id}">{e.project_name}</a>{:else if e.project_name}<span class="muted">{e.project_name}</span>{:else}<span class="muted">—</span>{/if}</td>
              </tr>
            {/each}
          {:else}<tr><td colspan="5" class="empty">{filtered ? 'Aucune action ne correspond aux filtres.' : 'Le journal est vide.'}</td></tr>{/each}
        </tbody>
      </table>
    </div>
  </section>
{/if}

<style>
  .jr-bar { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin-bottom: 12px; }
  .jr-bar select { width: auto; height: 40px; }
  .jr tr.day th { text-align: left; padding: 16px 8px 6px; font: 650 14px var(--font); letter-spacing: 0; text-transform: none; color: var(--fg); border-bottom: 1px solid var(--line); }
  .jr-d { display: block; }
</style>
