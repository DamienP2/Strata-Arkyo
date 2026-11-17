<script lang="ts">
  // Tâches du projet : trois vues (Kanban, Timeline, Liste), un seul formulaire d'édition.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import HeadActions from '../../components/HeadActions.svelte';
  import Seg from '../../components/Seg.svelte';
  import SortTh, { type Sort } from '../../components/SortTh.svelte';
  import TaskBoard from './TaskBoard.svelte';
  import TaskTimeline from './TaskTimeline.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route } from '../../lib/router.svelte';
  import { pageHead, plural } from '../../lib/pagehead.svelte';
  import { TASK_STATUS, PRIORITY, fdate, type ProjectView, type Task, type TaskStatus, type Priority } from '../../lib/projects';
  import type { Member } from '../../lib/types';
  import { STATUS_ORDER, isOpen, isLate, taskBody } from './tasks';

  let { view = $bindable(), reload }: { view: ProjectView; reload: () => Promise<void> } = $props();
  const writable = $derived(can('projets', 'ecriture'));
  let members = $state<Member[]>([]);
  api.get<{ members: Member[] }>('/members').then((r) => (members = r.members.filter((m) => m.active))).catch(() => {});
  let editing = $state<Partial<Task> & { title: string } | null>(null);
  let errors = $state<Record<string, string>>({});
  let busy = $state(false);
  let filter = $state<'open' | 'all'>('open');

  // Vue choisie : dans l'adresse (?vue=) pour la partager, et gardée d'une visite à l'autre.
  type Vue = 'kanban' | 'timeline' | 'liste';
  const VUES: Vue[] = ['kanban', 'timeline', 'liste'];
  const VUE_KEY = 'strata.tasks.vue';
  const asVue = (v: string | null | undefined): Vue | null => (VUES.includes(v as Vue) ? (v as Vue) : null);
  function storedVue(): Vue | null { try { return asVue(localStorage.getItem(VUE_KEY)); } catch { return null; } }
  let vue = $state<Vue>(asVue(route.query.get('vue')) ?? storedVue() ?? 'kanban');
  $effect(() => { const v = asVue(route.query.get('vue')); if (v && v !== untrack(() => vue)) vue = v; });
  function setVue(v: Vue) {
    try { localStorage.setItem(VUE_KEY, v); } catch { /* stockage indisponible */ }
    const u = new URL(location.href);
    u.searchParams.set('vue', v);
    // Changer de vue ne crée pas d'entrée d'historique.
    history.replaceState(history.state, '', u.pathname + u.search);
    route.query = new URLSearchParams(u.search);
  }

  // Tri de la liste, gardé d'une visite à l'autre (comme le filtre des projets).
  const SORT_KEY = 'strata.tasks.sort';
  let sort = $state<Sort>(readSort());
  function readSort(): Sort {
    try { const s = JSON.parse(localStorage.getItem(SORT_KEY) ?? 'null'); if (s && typeof s.key === 'string' && (s.dir === 1 || s.dir === -1)) return s; } catch { /* stockage indisponible */ }
    return { key: '', dir: 1 };
  }
  $effect(() => { const s = JSON.stringify(sort); try { localStorage.setItem(SORT_KEY, s); } catch { /* stockage indisponible */ } });

  const openCount = $derived(view.tasks.filter(isOpen).length);
  pageHead(() => ({ count: view.tasks.length ? `${plural(view.tasks.length, 'tâche')} · ${openCount} à traiter` : '' }));
  const PRIO_RANK = Object.keys(PRIORITY);
  const filtered = $derived(view.tasks.filter((t) => filter === 'all' || isOpen(t)));
  const shown = $derived.by(() => {
    if (!sort.key) return filtered;
    const k = sort.key, d = sort.dir;
    return [...filtered].sort((a, b) => {
      if (k === 'due') {
        // Sans échéance : toujours en fin de liste.
        if (!a.due || !b.due) return a.due ? -1 : b.due ? 1 : 0;
        return a.due.localeCompare(b.due) * d;
      }
      const rank = k === 'priority' ? PRIO_RANK : STATUS_ORDER;
      return (rank.indexOf(k === 'priority' ? a.priority : a.status) - rank.indexOf(k === 'priority' ? b.priority : b.status)) * d;
    });
  });
  const newTask = (status: TaskStatus = 'todo') => { errors = {}; editing = { title: '', status, priority: 'normal' }; };
  const edit = (t: Task) => { errors = {}; editing = { ...t }; };
  const nameOf = (id: string | null) => (id ? members.find((m) => m.id === id)?.name ?? '' : '');
  // La ligne entière ouvre la même édition que le crayon (sauf clic sur une commande de la ligne).
  function rowClick(e: MouseEvent, t: Task) {
    if (!writable || (e.target as Element).closest('button, select, a, input, label')) return;
    edit(t);
  }

  async function save() {
    errors = {};
    const t = editing!;
    if (t.start_date && t.due && t.start_date > t.due) { errors = { start_date: 'Le début doit précéder l’échéance.' }; return; }
    const body = { title: t.title, description: t.description || null, start_date: t.start_date || null, due: t.due || null, done_on: t.done_on ?? null, assignee_id: t.assignee_id || null, status: t.status ?? 'todo', priority: t.priority ?? 'normal' };
    busy = true;
    try {
      if (t.id) await api.patch(`/tasks/${t.id}`, { ...body, rev: t.rev });
      else await api.post(`/projects/${view.project.id}/tasks`, body);
      toast(t.id ? `Tâche « ${t.title} » enregistrée.` : `Tâche « ${t.title} » ajoutée.`); editing = null; await reload();
    } catch (x) { if (x instanceof ApiError) errors = Object.fromEntries(x.fields.map((f) => [f.field, f.message])); if (!(x instanceof ApiError) || !x.fields.length) fail(x); }
    finally { busy = false; }
  }

  /** Écrit une modification de la tâche (affichée tout de suite), puis propose de revenir en arrière. */
  async function change(t: Task, patch: Partial<Task>, message: string) {
    const before = taskBody(t);
    const i = view.tasks.findIndex((x) => x.id === t.id);
    if (i >= 0) view.tasks[i] = { ...t, ...patch };
    try {
      const after = await api.patch<Task>(`/tasks/${t.id}`, { ...before, ...patch, rev: t.rev });
      const j = view.tasks.findIndex((x) => x.id === t.id);
      if (j >= 0) view.tasks[j] = after;
      toast(message, 'ok', { run: async () => {
        const cur = view.tasks.find((x) => x.id === t.id) ?? after;
        try { await api.patch(`/tasks/${t.id}`, { ...before, rev: cur.rev }); await reload(); } catch (e) { fail(e); await reload(); return false; }
      } });
      await reload();
    } catch (e) { fail(e); await reload(); }
  }
  const setStatus = (t: Task, status: TaskStatus) => {
    if (status !== t.status) change(t, { status }, `Tâche « ${t.title} » passée à « ${TASK_STATUS[status].label} ».`);
  };
  const reschedule = (t: Task, start_date: string | null, due: string | null) =>
    change(t, { start_date, due }, `Tâche « ${t.title} » replanifiée${due ? ` au ${fdate(due)}` : ''}.`);

  // Suppression immédiate, annulable quelques secondes depuis la notification.
  async function remove(t: Task) {
    try {
      await api.del(`/tasks/${t.id}`); await reload();
      const body = taskBody(t);
      toast(`Tâche « ${t.title} » supprimée.`, 'ok', { run: async () => { try { await api.post(`/projects/${view.project.id}/tasks`, body); await reload(); } catch (e) { fail(e); return false; } } });
    } catch (e) { fail(e); }
  }
</script>

{#snippet status(t: Task)}
  {@const s = TASK_STATUS[t.status]}
  {#if writable}
    <label class="chip tk-st {s.chip}">{#if s.dot}<i class="dot" style="--c:{s.dot}"></i>{/if}<select value={t.status} onchange={(e) => setStatus(t, (e.target as HTMLSelectElement).value as TaskStatus)} aria-label="Statut de « {t.title} »">{#each Object.entries(TASK_STATUS) as [k, o]}<option value={k}>{o.label}</option>{/each}</select><Icon name="chevd" size="sm" /></label>
  {:else}<span class="chip {s.chip}">{#if s.dot}<i class="dot" style="--c:{s.dot}"></i>{/if}{s.label}</span>{/if}
{/snippet}
{#snippet newAction()}<button class="btn primary" onclick={() => newTask()}><Icon name="plus" size="sm" />Nouvelle tâche</button>{/snippet}

{#if writable && view.tasks.length}<HeadActions>{@render newAction()}</HeadActions>{/if}

{#if !view.tasks.length}
  <EmptyState icon="check" message="Aucune tâche" hint="Notez ici ce qu’il reste à faire sur le projet : une commande, un essai, un rendez-vous, avec une échéance et une personne." action={writable ? newAction : undefined} />
{:else}
  <div class="toolbar tk-bar">
    <Seg label="Vue des tâches" mode="tabs" controls="tk-vue" value={vue} onchange={setVue}
      options={[{ value: 'kanban', label: 'Kanban', icon: 'sheet' }, { value: 'timeline', label: 'Timeline', icon: 'cal' }, { value: 'liste', label: 'Liste', icon: 'list' }]} />
    {#if vue !== 'kanban'}
      <Seg label="Tâches affichées" bind:value={filter} options={[{ value: 'open', label: 'À traiter', count: openCount }, { value: 'all', label: 'Toutes', count: view.tasks.length }]} />
    {/if}
  </div>

  <div id="tk-vue-{vue}" role="tabpanel" aria-label={vue === 'kanban' ? 'Kanban' : vue === 'timeline' ? 'Timeline' : 'Liste'}>
    {#if vue === 'kanban'}
      <TaskBoard tasks={view.tasks} today={view.today} {writable} {nameOf} onedit={edit} onadd={newTask} onmove={setStatus} onremove={remove} />
    {:else if !filtered.length}
      <EmptyState icon="check" message="Rien à traiter" hint={`Les ${view.tasks.length} tâche${view.tasks.length > 1 ? 's' : ''} du projet ${view.tasks.length > 1 ? 'sont terminées ou validées' : 'est terminée ou validée'}.`}>
        {#snippet action()}<button class="btn" onclick={() => (filter = 'all')}>Voir toutes les tâches</button>{/snippet}
      </EmptyState>
    {:else if vue === 'timeline'}
      <TaskTimeline tasks={filtered} project={view.project} today={view.today} {writable} {nameOf} onedit={edit} onreschedule={reschedule} />
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
                <td data-l="Échéance"><span class="tk-due" class:urg={isLate(t, view.today)}>{t.due ? fdate(t.due) : 'Sans échéance'}{#if t.start_date}<span class="muted small">début {fdate(t.start_date)}</span>{/if}{#if t.done_on}<span class="muted small">fait le {fdate(t.done_on)}</span>{/if}</span></td>
                <td data-l="Priorité"><span class="chip {PRIORITY[t.priority].chip}">{PRIORITY[t.priority].label}</span></td>
                <td data-l="Statut">{@render status(t)}</td>
                <td class="acts">{#if writable}<div class="dl-actions"><button class="icon-btn sm" aria-label="Modifier « {t.title} »" title="Modifier « {t.title} »" onclick={() => edit(t)}><Icon name="pen" size="sm" /></button><button class="icon-btn sm danger" aria-label="Supprimer « {t.title} »" title="Supprimer « {t.title} »" onclick={() => remove(t)}><Icon name="trash" size="sm" /></button></div>{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table></div>
      </section>
    {/if}
  </div>
{/if}

{#if editing}
  <Dialog title={editing.id ? 'Modifier la tâche' : 'Nouvelle tâche'} eyebrow={view.project.name} onclose={() => (editing = null)} onsubmit={save} {busy} submitLabel={editing.id ? 'Enregistrer' : 'Ajouter'}>
    <div class="stack">
      <div class="field"><label for="t-title">Titre</label><input class="input" id="t-title" bind:value={editing.title} required maxlength="200" aria-invalid={!!errors.title} aria-describedby={errors.title ? 't-title-err' : undefined}><FieldError message={errors.title} for="t-title" /></div>
      <div class="field"><label for="t-desc">Description (facultatif)</label><textarea class="input" id="t-desc" rows="3" value={editing.description ?? ''} oninput={(e) => (editing!.description = (e.target as HTMLTextAreaElement).value)}></textarea></div>
      <div class="form-grid">
        <div class="field"><span class="flabel">Statut</span><Seg label="Statut" value={editing.status ?? 'todo'} onchange={(v) => (editing!.status = v)} options={STATUS_ORDER.map((k) => ({ value: k, label: TASK_STATUS[k].label }))} /></div>
        <div class="field"><span class="flabel">Priorité</span><Seg label="Priorité" value={editing.priority ?? 'normal'} onchange={(v) => (editing!.priority = v)} options={Object.entries(PRIORITY).map(([k, p]) => ({ value: k as Priority, label: p.label }))} /></div>
        <div class="field"><label for="t-start">Début (facultatif)</label><input class="input" type="date" id="t-start" aria-invalid={!!errors.start_date} aria-describedby={errors.start_date ? 't-start-err' : 't-start-help'} value={editing.start_date ?? ''} oninput={(e) => (editing!.start_date = (e.target as HTMLInputElement).value || null)}>{#if errors.start_date}<FieldError message={errors.start_date} for="t-start" />{:else}<button type="button" class="ux3-info" id="t-start-help" title="Avec une échéance, la tâche devient une barre sur la Timeline." aria-label="Avec une échéance, la tâche devient une barre sur la Timeline."><Icon name="info" size="sm" /></button>{/if}</div>
        <div class="field"><label for="t-due">Échéance</label><input class="input" type="date" id="t-due" aria-invalid={!!errors.due} aria-describedby={errors.due ? 't-due-err' : undefined} value={editing.due ?? ''} oninput={(e) => (editing!.due = (e.target as HTMLInputElement).value || null)}><FieldError message={errors.due} for="t-due" /></div>
        <div class="field"><label for="t-as">Assignée à</label><select class="input" id="t-as" aria-invalid={!!errors.assignee_id} aria-describedby={errors.assignee_id ? 't-as-err' : undefined} value={editing.assignee_id ?? ''} onchange={(e) => (editing!.assignee_id = (e.target as HTMLSelectElement).value || null)}><option value="">Personne</option>{#each members as m}<option value={m.id}>{m.name}</option>{/each}</select><FieldError message={errors.assignee_id} for="t-as" /></div>
      </div>
    </div>
  </Dialog>
{/if}

<style>
  .tk-bar { margin-bottom: 14px; }
  .tk-bar :global(.seg .ic) { margin-right: 6px; }
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
