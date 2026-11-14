<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import FieldError from '../../components/FieldError.svelte';
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

  const shown = $derived(view.tasks.filter((t) => filter === 'all' || t.status === 'todo' || t.status === 'in_progress'));
  const nameOf = (id: string | null) => members.find((m) => m.id === id)?.name ?? '';

  async function save(e: SubmitEvent) {
    e.preventDefault(); errors = {};
    const t = editing!;
    const body = { title: t.title, description: t.description || null, due: t.due || null, assignee_id: t.assignee_id || null, status: t.status ?? 'todo', priority: t.priority ?? 'normal' };
    try {
      if (t.id) await api.patch(`/tasks/${t.id}`, { ...body, rev: t.rev });
      else await api.post(`/projects/${view.project.id}/tasks`, body);
      toast(t.id ? 'Tâche enregistrée.' : 'Tâche ajoutée.'); editing = null; await reload();
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
      toast(`Tâche « ${t.title} » supprimée.`, 'ok', { label: 'Annuler', run: async () => { try { await api.post(`/projects/${view.project.id}/tasks`, { title, description, due, assignee_id, status, priority }); await reload(); } catch (e) { fail(e); } } });
    } catch (e) { fail(e); }
  }
  const late = (t: Task) => !!t.due && t.due < view.today && (t.status === 'todo' || t.status === 'in_progress');
</script>

<div class="toolbar" style="display:flex;gap:10px;align-items:center;flex-wrap:wrap">
  <div class="seg"><button class:on={filter === 'open'} onclick={() => (filter = 'open')}>À traiter</button><button class:on={filter === 'all'} onclick={() => (filter = 'all')}>Toutes ({view.tasks.length})</button></div>
  <span style="flex:1"></span>
  {#if writable}<button class="btn primary" onclick={() => (editing = { title: '', status: 'todo', priority: 'normal' })}><Icon name="plus" />Nouvelle tâche</button>{/if}
</div>

<!-- Téléphone : une carte par tâche (le tableau à 5 colonnes est illisible). -->
<ul class="task-cards">
  {#each shown as t (t.id)}
    <li class="card task-card">
      <div class="tc-head">
        <b class="tc-title">{t.title}</b>
        {#if writable}<div class="dl-actions"><button class="icon-btn sm" aria-label="Modifier « {t.title} »" onclick={() => (editing = { ...t })}><Icon name="pen" size="sm" /></button><button class="icon-btn sm danger" aria-label="Supprimer « {t.title} »" onclick={() => remove(t)}><Icon name="trash" size="sm" /></button></div>{/if}
      </div>
      {#if t.description}<p class="muted small tc-desc">{t.description}</p>{/if}
      <p class="tc-meta small">
        <span>{t.assignee_id ? nameOf(t.assignee_id) || '?' : 'Personne'}</span><span aria-hidden="true">·</span>
        <span class:urg={late(t)}>{t.due ? fdate(t.due) : 'Sans échéance'}</span><span aria-hidden="true">·</span>
        <span>Priorité {PRIORITY[t.priority].label.toLowerCase()}</span>
      </p>
      <div class="tc-foot">
        {#if writable}
          <label class="tc-st chip {TASK_STATUS[t.status].chip}"><select value={t.status} onchange={(e) => setStatus(t, (e.target as HTMLSelectElement).value as TaskStatus)} aria-label="Statut de « {t.title} »">{#each Object.entries(TASK_STATUS) as [k, s]}<option value={k}>{s.label}</option>{/each}</select><Icon name="chevd" size="sm" /></label>
        {:else}<span class="chip {TASK_STATUS[t.status].chip}">{TASK_STATUS[t.status].label}</span>{/if}
        {#if t.done_on}<span class="muted small">fait le {fdate(t.done_on)}</span>{/if}
      </div>
    </li>
  {:else}<li class="card empty">{filter === 'open' ? 'Rien à traiter.' : 'Aucune tâche.'}</li>{/each}
</ul>

<section class="card task-tbl" style="padding:6px 18px 10px">
  <div class="tbl-wrap"><table class="tbl">
    <thead><tr><th>Tâche</th><th>Assignée à</th><th>Échéance</th><th>Priorité</th><th>Statut</th><th></th></tr></thead>
    <tbody>
      {#each shown as t (t.id)}
        <tr>
          <td><b>{t.title}</b>{#if t.description}<br><span class="muted small">{t.description}</span>{/if}</td>
          <td>{#if t.assignee_id}<div style="display:flex;gap:8px;align-items:center"><Avatar name={nameOf(t.assignee_id) || '?'} size="sm" />{nameOf(t.assignee_id)}</div>{:else}<span class="muted">—</span>{/if}</td>
          <td class:urg={late(t)}>{fdate(t.due)}{#if t.done_on}<br><span class="muted small">fait le {fdate(t.done_on)}</span>{/if}</td>
          <td><span class="chip {PRIORITY[t.priority].chip}">{PRIORITY[t.priority].label}</span></td>
          <td>{#if writable}<select class="input" style="height:36px;width:auto" value={t.status} onchange={(e) => setStatus(t, (e.target as HTMLSelectElement).value as TaskStatus)} aria-label="Statut">{#each Object.entries(TASK_STATUS) as [k, s]}<option value={k}>{s.label}</option>{/each}</select>{:else}<span class="chip {TASK_STATUS[t.status].chip}">{TASK_STATUS[t.status].label}</span>{/if}</td>
          <td>{#if writable}<div class="dl-actions"><button class="btn sm ghost" title="Modifier" onclick={() => (editing = { ...t })}><Icon name="pen" size="sm" /></button><button class="btn sm ghost danger" title="Supprimer" onclick={() => remove(t)}><Icon name="trash" size="sm" /></button></div>{/if}</td>
        </tr>
      {:else}<tr><td colspan="6" class="empty">{filter === 'open' ? 'Rien à traiter.' : 'Aucune tâche.'}</td></tr>{/each}
    </tbody>
  </table></div>
</section>

{#if editing}
  <Dialog title={editing.id ? 'Modifier la tâche' : 'Nouvelle tâche'} eyebrow={view.project.name} onclose={() => (editing = null)}>
    <form id="taskf" class="stack" onsubmit={save}>
      <div class="field"><label for="t-title">Titre</label><input class="input" id="t-title" bind:value={editing.title} required maxlength="200"><FieldError message={errors.title} /></div>
      <div class="field"><label for="t-desc">Description</label><textarea class="input" id="t-desc" rows="3" style="height:auto;padding:12px 14px" value={editing.description ?? ''} oninput={(e) => (editing!.description = (e.target as HTMLTextAreaElement).value)}></textarea></div>
      <div class="form-grid">
        <div class="field"><label for="t-as">Assignée à</label><select class="input" id="t-as" value={editing.assignee_id ?? ''} onchange={(e) => (editing!.assignee_id = (e.target as HTMLSelectElement).value || null)}><option value="">Personne</option>{#each members as m}<option value={m.id}>{m.name}</option>{/each}</select><FieldError message={errors.assignee_id} /></div>
        <div class="field"><label for="t-due">Échéance</label><input class="input" type="date" id="t-due" value={editing.due ?? ''} oninput={(e) => (editing!.due = (e.target as HTMLInputElement).value || null)}><FieldError message={errors.due} /></div>
        <div class="field"><span class="flabel">Priorité</span><div class="seg">{#each Object.entries(PRIORITY) as [k, p]}<button type="button" class:on={editing.priority === k} onclick={() => (editing!.priority = k as Priority)}>{p.label}</button>{/each}</div></div>
      </div>
    </form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (editing = null)}>Annuler</button><button class="btn primary" type="submit" form="taskf">Enregistrer</button>{/snippet}
  </Dialog>
{/if}


<style>
  .task-cards { display: none; list-style: none; margin: 0; padding: 0; flex-direction: column; gap: 10px; }
  @media (max-width: 640px) {
    .task-tbl { display: none; }
    .task-cards { display: flex; }
  }
  .task-card { display: flex; flex-direction: column; gap: 6px; padding: 12px 14px; }
  .tc-head { display: flex; align-items: flex-start; gap: 8px; }
  .tc-title { flex: 1; min-width: 0; font-size: 15px; line-height: 1.3; overflow-wrap: anywhere; }
  .tc-head .icon-btn { width: 40px; height: 40px; }
  .tc-desc { margin: 0; }
  .tc-meta { display: flex; flex-wrap: wrap; gap: 2px 6px; margin: 0; color: var(--fg-2); }
  .tc-meta .urg { color: var(--err); font-weight: 600; }
  .tc-foot { display: flex; align-items: center; gap: 10px; }
  .tc-st { position: relative; display: inline-flex; align-items: center; gap: 4px; height: 32px; padding-right: 8px; }
  .tc-st select { appearance: none; border: 0; background: none; color: inherit; font: inherit; padding: 0 2px 0 0; height: 100%; }
  .tc-st select:focus-visible { outline: none; }
  .tc-st:focus-within { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
