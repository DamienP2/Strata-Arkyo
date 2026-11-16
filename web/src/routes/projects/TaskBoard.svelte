<script lang="ts">
  // Kanban des tâches : une colonne par statut. Glisser une carte vers une autre colonne change
  // son statut ; le menu ⋯ de la carte fait de même au clavier et au doigt.
  import Icon from '../../components/Icon.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Menu from '../../components/Menu.svelte';
  import { TASK_STATUS, PRIORITY, fdate, type Task, type TaskStatus } from '../../lib/projects';
  import { STATUS_ORDER, STATUS_COLOR, isLate, byPriorityThenDue } from './tasks';

  let { tasks, today, writable, nameOf, onedit, onadd, onmove, onremove }: {
    tasks: Task[]; today: string; writable: boolean;
    nameOf: (id: string | null) => string;
    onedit: (t: Task) => void;
    onadd: (s: TaskStatus) => void;
    onmove: (t: Task, s: TaskStatus) => void;
    onremove: (t: Task) => void;
  } = $props();

  const columns = $derived(STATUS_ORDER.map((s) => ({ s, list: tasks.filter((t) => t.status === s).sort(byPriorityThenDue) })));

  // Glisser-déposer HTML5 : la carte tirée et la colonne survolée.
  let dragId = $state<string | null>(null);
  let over = $state<TaskStatus | null>(null);
  const dragged = $derived(tasks.find((t) => t.id === dragId) ?? null);

  function dragStart(e: DragEvent, t: Task) {
    if (!writable || !e.dataTransfer) return;
    dragId = t.id;
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', t.id);
  }
  function dragEnd() { dragId = null; over = null; }
  function dragOver(e: DragEvent, s: TaskStatus) {
    if (!dragged || dragged.status === s) { if (over === s) over = null; return; }
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    over = s;
  }
  function dragLeave(e: DragEvent, s: TaskStatus) {
    // On ne quitte la colonne que si le pointeur en sort (pas en passant sur une carte).
    if (over === s && !(e.currentTarget as Element).contains(e.relatedTarget as Node | null)) over = null;
  }
  function drop(e: DragEvent, s: TaskStatus) {
    e.preventDefault();
    const t = dragged;
    dragEnd();
    if (t && t.status !== s) onmove(t, s);
  }
  // Clic sur la carte : même édition que le titre (sauf sur le menu).
  function cardClick(e: MouseEvent, t: Task) {
    if (!writable || (e.target as Element).closest('button, a, .menu-wrap')) return;
    onedit(t);
  }
</script>

<div class="kb" class:dragging={!!dragId}>
  {#each columns as { s, list } (s)}
    {@const st = TASK_STATUS[s]}
    <section class="kb-col" class:over={over === s} aria-labelledby="kb-h-{s}"
      ondragover={(e) => dragOver(e, s)} ondragleave={(e) => dragLeave(e, s)} ondrop={(e) => drop(e, s)}>
      <header class="kb-head">
        <i class="kb-dot" style="--c:{STATUS_COLOR[s]}"></i>
        <h3 id="kb-h-{s}">{st.label}</h3>
        <span class="muted mono">{list.length}</span>
      </header>
      <ul class="kb-list" aria-labelledby="kb-h-{s}">
        {#each list as t (t.id)}
          {@const late = isLate(t, today)}
          {@const who = nameOf(t.assignee_id)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
          <li class="kb-card" class:click={writable} class:lifted={dragId === t.id} draggable={writable}
            ondragstart={(e) => dragStart(e, t)} ondragend={dragEnd} onclick={(e) => cardClick(e, t)}>
            <div class="kb-top">
              {#if writable}<button type="button" class="kb-title" onclick={() => onedit(t)}>{t.title}</button>
              {:else}<span class="kb-title">{t.title}</span>{/if}
              {#if writable}
                <Menu label="Actions de « {t.title} »" icon="more" iconOnly btnClass="icon-btn sm" class="kb-menu">
                  {#snippet children()}
                    <button type="button" role="menuitem" onclick={() => onedit(t)}><Icon name="pen" size="sm" />Modifier</button>
                    <hr>
                    <span class="pm-sub kb-sub" id="kb-mv-{t.id}">Déplacer vers</span>
                    {#each STATUS_ORDER.filter((o) => o !== t.status) as o (o)}
                      <button type="button" role="menuitem" aria-describedby="kb-mv-{t.id}" onclick={() => onmove(t, o)}><i class="kb-dot" style="--c:{STATUS_COLOR[o]}"></i>{TASK_STATUS[o].label}</button>
                    {/each}
                    <hr>
                    <button type="button" role="menuitem" class="danger" onclick={() => onremove(t)}><Icon name="trash" size="sm" />Supprimer</button>
                  {/snippet}
                </Menu>
              {/if}
            </div>
            {#if t.priority !== 'normal' || t.due || who}
              <div class="kb-meta">
                {#if t.priority !== 'normal'}<span class="chip {PRIORITY[t.priority].chip}">{PRIORITY[t.priority].label}</span>{/if}
                {#if t.due}<span class="kb-due" class:late title={late ? 'Échéance dépassée' : 'Échéance'}><Icon name="cal" size="sm" />{late ? `En retard · ${fdate(t.due)}` : fdate(t.due)}</span>{/if}
                {#if who}<span class="kb-who"><Avatar name={who} size="sm" /></span>{/if}
              </div>
            {/if}
          </li>
        {/each}
      </ul>
      {#if !list.length}<p class="kb-none muted small">{over === s ? 'Déposer ici' : 'Aucune tâche'}</p>{/if}
      {#if writable}<button type="button" class="btn ghost sm kb-add" onclick={() => onadd(s)}><Icon name="plus" size="sm" />Ajouter</button>{/if}
    </section>
  {/each}
</div>

<style>
  .kb { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; align-items: start; }
  .kb-col { display: flex; flex-direction: column; gap: 8px; min-width: 0; padding: 10px; border-radius: var(--r-md); background: var(--surface-2); box-shadow: 0 0 0 1px var(--line); transition: background .12s, box-shadow .12s; }
  /* Colonne visée pendant le glisser : contour et fond d'accent. */
  .kb-col.over { background: var(--accent-soft); box-shadow: 0 0 0 1.5px var(--accent); }
  .kb-head { display: flex; align-items: center; gap: 8px; padding: 2px 4px; min-height: 28px; }
  .kb-head h3 { margin: 0; font: 650 var(--fs-md)/1.3 var(--font); color: var(--fg); }
  .kb-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--c); flex: none; }
  .kb-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; min-height: 8px; }
  .kb-card { display: flex; flex-direction: column; gap: 8px; padding: 10px 10px 10px 12px; border-radius: var(--r-sm); background: var(--card-bg, var(--surface)); box-shadow: var(--card-sh, 0 0 0 1px var(--line)); transition: box-shadow .12s, transform .12s, opacity .12s; }
  .kb-card.click { cursor: grab; }
  .kb-card.click:hover { box-shadow: 0 0 0 1px var(--line-2), 0 6px 16px -8px rgba(0, 0, 0, .25); }
  .kb-card.lifted { opacity: .45; cursor: grabbing; }
  .dragging .kb-card { cursor: grabbing; }
  .kb-top { display: flex; align-items: flex-start; gap: 6px; }
  .kb-title { flex: 1; min-width: 0; padding: 4px 0 0; border: 0; background: none; font: 600 var(--fs-md)/1.35 var(--font); color: var(--fg); text-align: left; overflow-wrap: anywhere; border-radius: 4px; }
  button.kb-title { cursor: pointer; }
  button.kb-title:hover { color: var(--accent); }
  button.kb-title:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .kb-top :global(.kb-menu) { flex: none; margin: -2px -4px 0 0; }
  .kb-sub { padding: 2px 10px 0; }
  .kb-meta { display: flex; align-items: center; flex-wrap: wrap; gap: 6px 10px; }
  .kb-due { display: inline-flex; align-items: center; gap: 5px; font-size: var(--fs-sm); color: var(--fg-3); white-space: nowrap; }
  .kb-due.late { color: var(--err); font-weight: 600; }
  .kb-who { margin-left: auto; display: inline-flex; }
  .kb-none { margin: 0; padding: 14px 8px; text-align: center; border-radius: var(--r-sm); box-shadow: inset 0 0 0 1px var(--line); }
  .kb-col.over .kb-none { color: var(--accent-2); box-shadow: inset 0 0 0 1.5px var(--accent); }
  .kb-add { justify-content: flex-start; width: 100%; }
  @media (max-width: 860px) {
    /* Téléphone et tablette : colonnes l'une sous l'autre (le menu ⋯ remplace le glisser). */
    .kb { grid-template-columns: minmax(0, 1fr); }
  }
</style>
