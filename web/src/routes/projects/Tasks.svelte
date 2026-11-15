<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Seg from '../../components/Seg.svelte';
  import SortTh, { type Sort } from '../../components/SortTh.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { TASK_STATUS, PRIORITY, fdate, type ProjectView, type Task, type TaskStatus, type Priority } from '../../lib/projects';
  import type { Member } from '../../lib/types';

  let { view = $bindable(), reload }: { view: ProjectView; reload: () => Promise<void> } = $props();
  const writable = $derived(can('projets', 'ecriture'));
  let members = $state<Member[]>([]);
  api.get<{ members: Member[] }>('/members').then((r) => (members = r.members.filter((m) => m.active))).catch(() => {});
  let editing = $state<Partial<Task> & { title: string } | null>(null);
  let errors = $state<Record<string, string>>({});
  let filter = $state<'open' | 'all'>('open');

  // Tri choisi, gardé d'une visite à l'autre (comme le filtre des projets).
  const SORT_KEY = 'strata.tasks.sort';
  let sort = $state<Sort>(readSort());
  function readSort(): Sort {
    try { const s = JSON.parse(localStorage.getItem(SORT_KEY) ?? 'null'); if (s && typeof s.key === 'string' && (s.dir === 1 || s.dir === -1)) return s; } catch { /* stockage indisponible */ }
    return { key: '', dir: 1 };
  }
  $effect(() => { const s = JSON.stringify(sort); try { localStorage.setItem(SORT_KEY, s); } catch { /* stockage indisponible */ } });

  const isOpen = (t: Task) => t.status === 'todo' || t.status === 'in_progress';
  const openCount = $derived(view.tasks.filter(isOpen).length);
  const PRIO_RANK = Object.keys(PRIORITY);
  const STATUS_RANK = Object.keys(TASK_STATUS);
  const shown = $derived.by(() => {
    const list = view.tasks.filter((t) => filter === 'all' || isOpen(t));
    if (!sort.key) return list;
    const k = sort.key, d = sort.dir;
    return [...list].sort((a, b) => {
      if (k === 'due') {
        // Sans échéance : toujours en fin de liste.
        if (!a.due || !b.due) return a.due ? -1 : b.due ? 1 : 0;
        return a.due.localeCompare(b.due) * d;
      }
      const rank = k === 'priority' ? PRIO_RANK : STATUS_RANK;
      return (rank.indexOf(k === 'priority' ? a.priority : a.status) - rank.indexOf(k === 'priority' ? b.priority : b.status)) * d;
    });
  });
  const newTask = () => (editing = { title: '', status: 'todo', priority: 'normal' });
  const edit = (t: Task) => (editing = { ...t });
  const nameOf = (id: string | null) => members.find((m) => m.id === id)?.name ?? '';
  // La ligne entière ouvre la même édition que le crayon (sauf clic sur une commande de la ligne).
  function rowClick(e: MouseEvent, t: Task) {
    if (!writable || (e.target as Element).closest('button, select, a, input, label')) return;
    edit(t);
  }

  async function save() {
    errors = {};
    const t = editing!;
    const body = { title: t.title, description: t.description || null, due: t.due || null, assignee_id: t.assignee_id || null, status: t.status ?? 'todo', priority: t.priority ?? 'normal' };
    try {
      if (t.id) await api.patch(`/tasks/${t.id}`, { ...body, rev: t.rev });
      else await api.post(`/projects/${view.project.id}/tasks`, body);
      toast(t.id ? `Tâche « ${t.title} » enregistrée.` : `Tâche « ${t.title} » ajoutée.`); editing = null; await reload();
    } catch (x) { if (x instanceof ApiError) errors = Object.fromEntries(x.fields.map((f) => [f.field, f.message])); if (!(x instanceof ApiError) || !x.fields.length) fail(x); }
  }
  async function setStatus(t: Task, status: TaskStatus) {
    try { await api.patch(`/tasks/${t.id}`, { ...t, status, rev: t.rev }); await reload(); } catch (e) { fail(e); await reload(); }
  }
  // Suppression immédiate, annulable quelques secondes depuis la notification.
  async function remove(t: Task) {
    try {
      await api.del(`/tasks/${t.id}`); await reload();
      const { title, description, due, assignee_id, status, priority } = t;
      toast(`Tâche « ${t.title} » supprimée.`, 'ok', { run: async () => { try { await api.post(`/projects/${view.project.id}/tasks`, { title, description, due, assignee_id, status, priority }); await reload(); } catch (e) { fail(e); return false; } } });
    } catch (e) { fail(e); }
  }
  const late = (t: Task) => !!t.due && t.due < view.today && isOpen(t);
</script>

{#snippet status(t: Task)}
  {@const s = TASK_STATUS[t.status]}
  {#if writable}
    <label class="chip tk-st {s.chip}">{#if s.dot}<i class="dot" style="--c:{s.dot}"></i>{/if}<select value={t.status} onchange={(e) => setStatus(t, (e.target as HTMLSelectElement).value as TaskStatus)} aria-label="Statut de « {t.title} »">{#each Object.entries(TASK_STATUS) as [k, o]}<option value={k}>{o.label}</option>{/each}</select><Icon name="chevd" size="sm" /></label>
  {:else}<span class="chip {s.chip}">{#if s.dot}<i class="dot" style="--c:{s.dot}"></i>{/if}{s.label}</span>{/if}
{/snippet}

<div class="toolbar">
  <Seg label="Tâches affichées" bind:value={filter} options={[{ value: 'open', label: 'À traiter', count: openCount }, { value: 'all', label: 'Toutes', count: view.tasks.length }]} />
  <span class="grow"></span>
  {#if writable && view.tasks.length}<button class="btn primary" onclick={newTask}><Icon name="plus" />Nouvelle tâche</button>{/if}
</div>

{#if !shown.length}
  {#if view.tasks.length}
    <EmptyState icon="check" message="Rien à traiter" hint={`Les ${view.tasks.length} tâche${view.tasks.length > 1 ? 's' : ''} du projet ${view.tasks.length > 1 ? 'sont terminées ou validées' : 'est terminée ou validée'}.`}>
      {#snippet action()}<button class="btn" onclick={() => (filter = 'all')}>Voir toutes les tâches</button>{/snippet}
    </EmptyState>
  {:else}
    <EmptyState icon="check" message="Aucune tâche" hint="Notez ici ce qu’il reste à faire sur le projet : une commande, un essai, un rendez-vous, avec une échéance et une personne." action={writable ? newAction : undefined} />
    {#snippet newAction()}<button class="btn primary" onclick={newTask}><Icon name="plus" size="sm" />Nouvelle tâche</button>{/snippet}
  {/if}
{:else}
<!-- Téléphone : .tbl.stack présente chaque tâche en carte. -->
<section class="card tbl-card">
  <div class="tbl-wrap"><table class="tbl stack">
    <thead><tr><th>Tâche</th><th>Assignée à</th><SortTh label="Échéance" key="due" bind:sort /><SortTh label="Priorité" key="priority" bind:sort /><SortTh label="Statut" key="status" bind:sort /><th><span class="sr-only">Actions</span></th></tr></thead>
    <tbody>
      {#each shown as t (t.id)}
        <tr class:click={writable} onclick={(e) => rowClick(e, t)}>
          <td class="first tk-t">
            {#if writable}<button type="button" class="cell-link tk-open" onclick={() => edit(t)}>{t.title}</button>{:else}<b>{t.title}</b>{/if}
            {#if t.description}<span class="muted small">{t.description}</span>{/if}
          </td>
          <td data-l="Assignée à">{#if t.assignee_id}<span class="tk-who"><Avatar name={nameOf(t.assignee_id) || '?'} size="sm" />{nameOf(t.assignee_id)}</span>{:else}<span class="muted">Personne</span>{/if}</td>
          <td data-l="Échéance"><span class="tk-due" class:urg={late(t)}>{t.due ? fdate(t.due) : 'Sans échéance'}{#if t.done_on}<span class="muted small">fait le {fdate(t.done_on)}</span>{/if}</span></td>
          <td data-l="Priorité"><span class="chip {PRIORITY[t.priority].chip}">{PRIORITY[t.priority].label}</span></td>
          <td data-l="Statut">{@render status(t)}</td>
          <td class="acts">{#if writable}<div class="dl-actions"><button class="icon-btn sm" aria-label="Modifier « {t.title} »" title="Modifier « {t.title} »" onclick={() => edit(t)}><Icon name="pen" size="sm" /></button><button class="icon-btn sm danger" aria-label="Supprimer « {t.title} »" title="Supprimer « {t.title} »" onclick={() => remove(t)}><Icon name="trash" size="sm" /></button></div>{/if}</td>
        </tr>
      {/each}
    </tbody>
  </table></div>
</section>
{/if}

{#if editing}
  <Dialog title={editing.id ? 'Modifier la tâche' : 'Nouvelle tâche'} eyebrow={view.project.name} onclose={() => (editing = null)} onsubmit={save} submitLabel={editing.id ? 'Enregistrer' : 'Ajouter'}>
    <div class="stack">
      <div class="field"><label for="t-title">Titre</label><input class="input" id="t-title" bind:value={editing.title} required maxlength="200" aria-invalid={!!errors.title} aria-describedby={errors.title ? 't-title-err' : undefined}><FieldError message={errors.title} for="t-title" /></div>
      <div class="field"><label for="t-desc">Description (facultatif)</label><textarea class="input" id="t-desc" rows="3" value={editing.description ?? ''} oninput={(e) => (editing!.description = (e.target as HTMLTextAreaElement).value)}></textarea></div>
      <div class="form-grid">
        <div class="field"><label for="t-as">Assignée à</label><select class="input" id="t-as" aria-invalid={!!errors.assignee_id} aria-describedby={errors.assignee_id ? 't-as-err' : undefined} value={editing.assignee_id ?? ''} onchange={(e) => (editing!.assignee_id = (e.target as HTMLSelectElement).value || null)}><option value="">Personne</option>{#each members as m}<option value={m.id}>{m.name}</option>{/each}</select><FieldError message={errors.assignee_id} for="t-as" /></div>
        <div class="field"><label for="t-due">Échéance</label><input class="input" type="date" id="t-due" aria-invalid={!!errors.due} aria-describedby={errors.due ? 't-due-err' : undefined} value={editing.due ?? ''} oninput={(e) => (editing!.due = (e.target as HTMLInputElement).value || null)}><FieldError message={errors.due} for="t-due" /></div>
        <div class="field"><span class="flabel">Priorité</span><Seg label="Priorité" value={editing.priority ?? 'normal'} onchange={(v) => (editing!.priority = v)} options={Object.entries(PRIORITY).map(([k, p]) => ({ value: k as Priority, label: p.label }))} /></div>
      </div>
    </div>
  </Dialog>
{/if}

<style>
  .grow { flex: 1; }
  .tk-t { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; }
  .tk-open { border: 0; background: none; padding: 0; font: inherit; font-weight: 600; text-align: left; cursor: pointer; border-radius: 4px; }
  .tk-open:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .tk-who { display: inline-flex; gap: 8px; align-items: center; }
  .tk-due { display: inline-flex; flex-direction: column; }
  .tk-due.urg { color: var(--err); font-weight: 600; }
  /* Statut modifiable : une pastille qui contient la liste de choix. */
  .tk-st { position: relative; height: 30px; padding-right: 8px; cursor: pointer; }
  .tk-st select { appearance: none; border: 0; background: none; color: inherit; font: inherit; padding: 0 2px 0 0; height: 100%; cursor: pointer; }
  .tk-st select:focus-visible { outline: none; }
  .tk-st:focus-within { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
