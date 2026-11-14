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

<section class="card" style="padding:6px 18px 10px">
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
