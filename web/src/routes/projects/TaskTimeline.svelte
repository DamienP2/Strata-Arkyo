<script lang="ts">
  // Timeline des tâches : axe des jours, périodes du projet en fond, une ligne par tâche
  // (barre du début à l'échéance, ou jalon à l'échéance). Défilement horizontal interne.
  import { tick } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Seg from '../../components/Seg.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import { TASK_STATUS, PRIORITY, fdate, daysBetween, type Project, type Task } from '../../lib/projects';
  import { STATUS_ORDER, STATUS_COLOR, isLate, addDays, weekday, isoWeek } from './tasks';

  let { tasks, project, today, writable, nameOf, onedit, onreschedule }: {
    tasks: Task[]; project: Project; today: string; writable: boolean;
    nameOf: (id: string | null) => string;
    onedit: (t: Task) => void;
    /** Décalage par glisser : nouvelles dates de début (ou null) et d'échéance. */
    onreschedule: (t: Task, start: string | null, due: string | null) => void;
  } = $props();

  // Zoom gardé d'une visite à l'autre.
  const ZOOM_KEY = 'strata.tasks.zoom';
  let zoom = $state<'week' | 'month'>(readZoom());
  function readZoom(): 'week' | 'month' { try { return localStorage.getItem(ZOOM_KEY) === 'month' ? 'month' : 'week'; } catch { return 'week'; } }
  $effect(() => { const z = zoom; try { localStorage.setItem(ZOOM_KEY, z); } catch { /* stockage indisponible */ } });
  const dw = $derived(zoom === 'week' ? 40 : 14);

  type Row = { t: Task; kind: 'bar' | 'ms'; from: string; to: string };
  const rows = $derived.by<Row[]>(() => tasks
    .filter((t) => t.due || t.start_date)
    .map((t): Row => t.start_date && t.due && t.start_date <= t.due
      ? { t, kind: 'bar', from: t.start_date, to: t.due }
      : { t, kind: 'ms', from: (t.due ?? t.start_date)!, to: (t.due ?? t.start_date)! })
    .sort((a, b) => a.from.localeCompare(b.from) || a.to.localeCompare(b.to) || STATUS_ORDER.indexOf(a.t.status) - STATUS_ORDER.indexOf(b.t.status)));
  const undated = $derived(tasks.filter((t) => !t.due && !t.start_date));

  const periods = $derived([
    { key: 'prep', label: 'Préparation', from: project.planning_start, to: project.planning_end ?? project.planning_start, c: 'var(--st-prep)' },
    { key: 'shoot', label: 'Exploitation', from: project.shoot_start ?? project.commissioning, to: project.shoot_end ?? project.shoot_start ?? project.commissioning, c: 'var(--st-cours)' },
  ].filter((p): p is { key: string; label: string; from: string; to: string; c: string } => !!p.from && !!p.to && p.from <= p.to));

  // Étendue : du lundi avant la première date au dimanche après la dernière, avec de la marge.
  const range = $derived.by(() => {
    const ds = [today, ...periods.flatMap((p) => [p.from, p.to]), ...rows.flatMap((r) => [r.from, r.to])].sort();
    let a = addDays(ds[0]!, -7), b = addDays(ds[ds.length - 1]!, zoom === 'week' ? 14 : 28);
    a = addDays(a, -weekday(a));
    b = addDays(b, 6 - weekday(b));
    const min = zoom === 'week' ? 8 * 7 : 20 * 7;
    if (daysBetween(a, b) + 1 < min) b = addDays(a, min - 1);
    return { start: a, days: daysBetween(a, b) + 1 };
  });
  const x = (d: string) => daysBetween(range.start, d) * dw;
  const W = $derived(range.days * dw);
  const days = $derived(Array.from({ length: range.days }, (_, i) => addDays(range.start, i)));
  const weeks = $derived(days.filter((_, i) => i % 7 === 0));
  // Mois visibles (vue Mois) : premier jour affiché de chaque mois et nombre de jours.
  const months = $derived.by(() => {
    const out: { d: string; n: number }[] = [];
    for (const d of days) { const last = out[out.length - 1]; if (last && last.d.slice(0, 7) === d.slice(0, 7)) last.n++; else out.push({ d, n: 1 }); }
    return out;
  });
  const DOW = ['L', 'M', 'M', 'J', 'V', 'S', 'D'];
  const todayIn = $derived(today >= range.start && daysBetween(range.start, today) < range.days);

  // Ouverture et changement de zoom : la semaine en cours reste en vue.
  let scroller = $state<HTMLDivElement>();
  async function toToday(smooth = false) {
    await tick();
    if (!scroller) return;
    // Une semaine de recul, ou un quart de la zone visible sur un écran étroit.
    const lw = scroller.querySelector<HTMLElement>('.tl-label')?.offsetWidth ?? 0;
    const back = Math.min(7 * dw, (scroller.clientWidth - lw) / 4);
    scroller.scrollTo({ left: Math.max(0, x(today) - back), behavior: smooth ? 'smooth' : 'auto' });
  }
  // Défilement courant : les libellés de mois et de période restent lisibles au bord gauche.
  let sl = $state(0);
  const pad = (from: string, width: number) => 8 + Math.max(0, Math.min(sl - x(from), width - 120));
  $effect(() => { void dw; void range.start; toToday(); });

  // Glisser une barre ou un jalon (souris ou stylet) : décalage par jours entiers, aperçu en direct.
  let drag = $state<{ id: string; x0: number; n: number; pid: number } | null>(null);
  let dragged = false;
  function down(e: PointerEvent, r: Row) {
    if (!writable || e.button !== 0 || e.pointerType === 'touch') return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { id: r.t.id, x0: e.clientX, n: 0, pid: e.pointerId };
  }
  function move(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pid) return;
    drag.n = Math.round((e.clientX - drag.x0) / dw);
  }
  function up(e: PointerEvent, r: Row) {
    if (!drag || e.pointerId !== drag.pid) return;
    const n = drag.n;
    drag = null;
    if (!n) return; // Un clic sans mouvement n'écrit rien (le clic ouvre l'édition).
    dragged = true;
    const t = r.t;
    onreschedule(t, t.start_date ? addDays(t.start_date, n) : null, t.due ? addDays(t.due, n) : null);
  }
  function cancel() { drag = null; }
  function click(t: Task) {
    if (dragged) { dragged = false; return; }
    if (writable) onedit(t);
  }
  const shift = (r: Row) => (drag?.id === r.t.id ? drag.n : 0);
  const span = (r: Row) => (r.kind === 'bar' ? `${fdate(r.from)} → ${fdate(r.to)}` : r.t.due ? `échéance ${fdate(r.to)}` : `début ${fdate(r.from)}`);
</script>

<div class="tl-bar">
  <div class="tl-legend" aria-hidden="true">
    {#each STATUS_ORDER as s (s)}<span><i class="tl-dot" style="--c:{STATUS_COLOR[s]}"></i>{TASK_STATUS[s].label}</span>{/each}
    <span><i class="tl-dot" style="--c:var(--err)"></i>En retard</span>
    <span><i class="tl-dia"></i>Échéance seule</span>
  </div>
  <span class="grow"></span>
  <button type="button" class="btn sm" onclick={() => toToday(true)} disabled={!todayIn}>Aujourd’hui</button>
  <Seg label="Échelle" bind:value={zoom} options={[{ value: 'week', label: 'Semaine' }, { value: 'month', label: 'Mois' }]} />
</div>

<section class="card flush tl-card">
  {#if !rows.length}
    <EmptyState card={false} icon="cal" message="Aucune tâche datée" hint="Donnez une échéance à une tâche, et si besoin une date de début, pour la placer sur la Timeline." />
  {:else}
  <div class="tl-scroll" bind:this={scroller} onscroll={() => (sl = scroller!.scrollLeft)}>
    <div class="tl-grid" style="--dw:{dw}px;--w:{W}px">
      <!-- Axe -->
      <div class="tl-row tl-head">
        <div class="tl-label tl-corner"><span class="muted small">{rows.length} tâche{rows.length > 1 ? 's' : ''} datée{rows.length > 1 ? 's' : ''}</span></div>
        <div class="tl-axis">
          <div class="tl-ax1">
            {#if zoom === 'week'}
              {#each weeks as w (w)}<span style="left:{x(w)}px;width:{7 * dw}px;padding-left:{pad(w, 7 * dw)}px">Sem. {isoWeek(w)} · {fdate(w)}</span>{/each}
            {:else}
              {#each months as m (m.d)}<span style="left:{x(m.d)}px;width:{m.n * dw}px;padding-left:{pad(m.d, m.n * dw)}px">{fdate(m.d, { month: 'long', year: 'numeric' })}</span>{/each}
            {/if}
          </div>
          <div class="tl-ax2">
            {#if zoom === 'week'}
              {#each days as d (d)}<span class="mono" class:we={weekday(d) > 4} class:now={d === today} style="left:{x(d)}px;width:{dw}px"><b>{DOW[weekday(d)]}</b>{+d.slice(8)}</span>{/each}
            {:else}
              {#each weeks as w (w)}<span class="mono" style="left:{x(w)}px;width:{7 * dw}px">{+w.slice(8)}</span>{/each}
            {/if}
          </div>
          {#if periods.length}
            <div class="tl-ax3">
              {#each periods as p (p.key)}<span class="tl-per" style="left:{x(p.from)}px;width:{(daysBetween(p.from, p.to) + 1) * dw}px;padding-left:{pad(p.from, (daysBetween(p.from, p.to) + 1) * dw)}px;--c:{p.c}" title="{p.label} : {fdate(p.from)} → {fdate(p.to)}">{p.label}</span>{/each}
            </div>
          {/if}
          {#if todayIn}<span class="tl-today-tag" style="left:{x(today) + dw / 2}px">Aujourd’hui</span>{/if}
        </div>
      </div>
      <!-- Lignes -->
      <div class="tl-body">
        <div class="tl-bg" aria-hidden="true">
          {#each periods as p (p.key)}<i class="tl-band" style="left:{x(p.from)}px;width:{(daysBetween(p.from, p.to) + 1) * dw}px;--c:{p.c}"></i>{/each}
          {#if todayIn}<i class="tl-now" style="left:{x(today) + dw / 2}px"></i>{/if}
        </div>
        {#each rows as r (r.t.id)}
          {@const t = r.t}
          {@const late = isLate(t, today)}
          {@const c = late ? 'var(--err)' : STATUS_COLOR[t.status]}
          {@const n = shift(r)}
          {@const who = nameOf(t.assignee_id)}
          <div class="tl-row">
            <div class="tl-label">
              <i class="tl-dot" style="--c:{STATUS_COLOR[t.status]}" title={TASK_STATUS[t.status].label}></i>
              <span class="tl-txt">
                {#if writable}<button type="button" class="tl-name" onclick={() => onedit(t)}>{t.title}</button>{:else}<span class="tl-name">{t.title}</span>{/if}
                <span class="tl-sub small" class:late>{late ? `En retard · ${span(r)}` : span(r)}</span>
              </span>
              {#if who}<Avatar name={who} size="sm" />{/if}
            </div>
            <div class="tl-track">
              <button type="button" class="tl-item {r.kind}" class:done={!late && (t.status === 'done' || t.status === 'validated')} class:grab={writable} class:moving={drag?.id === t.id}
                style="left:{x(r.from) + n * dw}px;{r.kind === 'bar' ? `width:${(daysBetween(r.from, r.to) + 1) * dw}px;` : ''}--c:{c}"
                aria-label="{t.title} : {TASK_STATUS[t.status].label}, {span(r)}{late ? ', en retard' : ''}{t.priority !== 'normal' ? `, priorité ${PRIORITY[t.priority].label.toLowerCase()}` : ''}"
                title="{t.title} · {span(r)}{writable ? ' · glisser pour décaler' : ''}"
                tabindex={writable ? 0 : -1}
                onpointerdown={(e) => down(e, r)} onpointermove={move} onpointerup={(e) => up(e, r)} onpointercancel={cancel} onclick={() => click(t)}></button>
              {#if n}<span class="tl-tip mono" style="left:{x(r.to) + (n + 1) * dw + 8}px">{r.kind === 'bar' ? `${fdate(addDays(r.from, n))} → ${fdate(addDays(r.to, n))}` : fdate(addDays(r.to, n))}</span>{/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  </div>
  {/if}
</section>

{#if undated.length}
  <section class="card compact tl-undated" aria-labelledby="tl-nd">
    <h3 id="tl-nd">Sans date <span class="muted mono">{undated.length}</span></h3>
    <ul>
      {#each undated as t (t.id)}
        {@const who = nameOf(t.assignee_id)}
        <li>
          <i class="tl-dot" style="--c:{STATUS_COLOR[t.status]}" title={TASK_STATUS[t.status].label}></i>
          {#if writable}<button type="button" class="tl-name" onclick={() => onedit(t)}>{t.title}</button>{:else}<span class="tl-name">{t.title}</span>{/if}
          <span class="muted small">{TASK_STATUS[t.status].label}</span>
          {#if t.priority !== 'normal'}<span class="chip {PRIORITY[t.priority].chip}">{PRIORITY[t.priority].label}</span>{/if}
          {#if who}<Avatar name={who} size="sm" />{/if}
          {#if writable}<button type="button" class="icon-btn sm" aria-label="Dater « {t.title} »" title="Dater « {t.title} »" onclick={() => onedit(t)}><Icon name="cal" size="sm" /></button>{/if}
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .grow { flex: 1; }
  .tl-bar { display: flex; align-items: center; flex-wrap: wrap; gap: 8px 12px; margin-bottom: 12px; }
  .tl-legend { display: flex; flex-wrap: wrap; gap: 4px 14px; font-size: var(--fs-sm); color: var(--fg-3); }
  .tl-legend span { display: inline-flex; align-items: center; gap: 6px; }
  .tl-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--c); flex: none; }
  .tl-dia { width: 8px; height: 8px; transform: rotate(45deg); background: var(--fg-3); flex: none; border-radius: 1px; }

  /* Défilement horizontal limité à la carte : la page ne défile jamais de côté. */
  .tl-card { min-width: 0; max-width: 100%; overflow: hidden; }
  .tl-scroll { overflow-x: auto; overflow-y: hidden; overscroll-behavior-x: contain; --lw: 260px; }
  .tl-grid { width: calc(var(--lw) + var(--w)); position: relative; }
  .tl-row { display: flex; min-height: 48px; }
  .tl-row + .tl-row { border-top: 1px solid var(--line); }
  .tl-label { position: sticky; left: 0; z-index: 3; flex: none; width: var(--lw); display: flex; align-items: center; gap: 10px; padding: 6px 12px 6px 16px; background: var(--surface); box-shadow: 1px 0 0 var(--line); }
  .tl-txt { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .tl-name { display: block; max-width: 100%; padding: 0; border: 0; background: none; font: 600 var(--fs-md)/1.35 var(--font); color: var(--fg); text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; border-radius: 4px; }
  button.tl-name { cursor: pointer; }
  button.tl-name:hover { color: var(--accent); }
  button.tl-name:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .tl-sub { color: var(--fg-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tl-sub.late { color: var(--err); font-weight: 600; }

  /* Axe */
  .tl-head { position: relative; z-index: 4; border-bottom: 1px solid var(--line); }
  .tl-corner { align-items: flex-end; padding-bottom: 8px; }
  .tl-axis { position: relative; width: var(--w); flex: none; display: flex; flex-direction: column; padding-bottom: 4px; }
  .tl-ax1, .tl-ax2, .tl-ax3 { position: relative; height: 24px; }
  .tl-ax1 span, .tl-ax2 span { position: absolute; top: 0; height: 100%; display: flex; align-items: center; overflow: hidden; white-space: nowrap; }
  .tl-ax1 span { padding: 0 8px; font: 600 var(--fs-xs)/1 var(--font); color: var(--fg-2); box-shadow: inset 1px 0 0 var(--line); }
  .tl-ax2 span { justify-content: center; gap: 3px; font-size: var(--fs-2xs); color: var(--fg-3); }
  .tl-ax2 span b { font: 600 var(--fs-2xs)/1 var(--font); color: var(--fg-3); }
  .tl-ax2 span.we { color: var(--fg-4); }
  .tl-ax2 span.now { color: var(--accent); font-weight: 700; }
  .tl-ax2 span.now b { color: var(--accent); }
  .tl-ax3 { height: 22px; }
  .tl-per { position: absolute; top: 2px; height: 18px; display: flex; align-items: center; padding: 0 8px; border-radius: 6px; overflow: hidden; white-space: nowrap; font: 600 var(--fs-2xs)/1 var(--font); color: var(--fg-2); background: color-mix(in srgb, var(--c) 18%, var(--surface)); box-shadow: inset 3px 0 0 var(--c); }
  .tl-today-tag { position: absolute; bottom: -10px; transform: translateX(-50%); z-index: 2; height: 18px; padding: 0 7px; display: flex; align-items: center; border-radius: 999px; background: var(--accent); color: var(--primary-fg); font: 700 var(--fs-2xs)/1 var(--font); white-space: nowrap; pointer-events: none; }

  /* Corps : week-ends et lundis par dégradé (l'étendue commence un lundi), périodes, aujourd'hui. */
  .tl-body { position: relative; }
  .tl-bg { position: absolute; top: 0; bottom: 0; left: var(--lw); width: var(--w); pointer-events: none;
    background:
      repeating-linear-gradient(90deg, var(--line) 0 1px, transparent 1px calc(var(--dw) * 7)),
      repeating-linear-gradient(90deg, transparent 0 calc(var(--dw) * 5), color-mix(in srgb, var(--fg) 4%, transparent) calc(var(--dw) * 5) calc(var(--dw) * 7)); }
  .tl-band { position: absolute; top: 0; bottom: 0; background: color-mix(in srgb, var(--c) 9%, transparent); box-shadow: inset 1px 0 0 color-mix(in srgb, var(--c) 45%, transparent), inset -1px 0 0 color-mix(in srgb, var(--c) 45%, transparent); }
  .tl-now { position: absolute; top: 0; bottom: 0; width: 2px; margin-left: -1px; background: var(--accent); }
  .tl-track { position: relative; flex: none; width: var(--w); }
  .tl-item { position: absolute; top: 50%; padding: 0; border: 0; z-index: 1; }
  .tl-item.bar { height: 20px; margin-top: -10px; border-radius: 6px; background: color-mix(in srgb, var(--c) 32%, var(--surface)); box-shadow: inset 0 0 0 1.5px var(--c); }
  .tl-item.ms { width: 14px; height: 14px; margin: -7px 0 0 calc(var(--dw) / 2 - 7px); transform: rotate(45deg); border-radius: 2px; background: var(--c); box-shadow: 0 0 0 2px var(--surface); }
  .tl-item.done { opacity: .6; }
  .tl-item.grab { cursor: grab; touch-action: pan-y; }
  .tl-item.moving { cursor: grabbing; opacity: 1; box-shadow: inset 0 0 0 1.5px var(--c), 0 6px 16px -8px rgba(0, 0, 0, .35); z-index: 2; }
  .tl-item.ms.moving { box-shadow: 0 0 0 2px var(--surface), 0 6px 16px -8px rgba(0, 0, 0, .35); }
  .tl-item.bar:hover { box-shadow: inset 0 0 0 2px var(--c); }
  .tl-item.ms:hover { box-shadow: 0 0 0 2px var(--surface), 0 0 0 3.5px var(--c); }
  .tl-item:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .tl-tip { position: absolute; top: 50%; transform: translateY(-50%); z-index: 3; padding: 3px 7px; border-radius: 6px; background: var(--fg); color: var(--bg); font-size: var(--fs-2xs); white-space: nowrap; pointer-events: none; }

  /* Sans date */
  .tl-undated { margin-top: 12px; }
  .tl-undated h3 { margin: 0 0 6px; font: 650 var(--fs-lg)/1.25 var(--font); display: flex; align-items: center; gap: 8px; }
  .tl-undated ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; }
  .tl-undated li { display: flex; align-items: center; gap: 10px; min-height: 40px; padding: 2px 0; }
  .tl-undated li + li { border-top: 1px solid var(--line); }
  .tl-undated .tl-name { flex: 0 1 auto; min-width: 0; }
  .tl-undated li > .muted { margin-right: auto; white-space: nowrap; }
  @media (max-width: 860px) {
    .tl-scroll { --lw: 150px; }
    .tl-label { padding-left: 12px; gap: 8px; }
    .tl-label :global(.av) { display: none; }
  }
</style>
