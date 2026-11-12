<script lang="ts">
  // Accueil (section 7.5) : prochaine captation, indicateurs, projets à venir, activité.
  import Icon from '../components/Icon.svelte';
  import Ill from '../components/Ill.svelte';
  import Vu from '../components/Vu.svelte';
  import Avatar from '../components/Avatar.svelte';
  import { api } from '../lib/api';
  import { session, can } from '../lib/session.svelte';
  import { nf, relative } from '../lib/format';
  import { STATUS, STATUS_ORDER, REGIE, fdate, urgency, type Project, type Status } from '../lib/projects';

  interface Dash {
    today: string; projects_total: number; projects_active: number; tasks_total: number; tasks_done: number; tasks_pct: number; completion_avg: number;
    next: { project: Project; completion: number; days: number } | null;
    upcoming: { project: Project; completion: number; date: string; days: number }[];
    recent: { project: Project; completion: number }[];
    by_status: Partial<Record<Status, number>>;
  }
  const me = $derived(session.me!);
  const first = $derived(me.account.name.split(/\s+/)[0]);
  const today = new Date().toLocaleDateString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
  let dash = $state<Dash | null>(null);
  let catalog = $state<number | null>(null);
  let site = $state<{ version: string; available: string | null; lastBackup: string | null } | null>(null);

  if (can('projets', 'lecture')) api.get<Dash>('/dashboard').then((d) => (dash = d)).catch(() => {});
  if (can('catalogue', 'lecture')) api.get<{ total: number }>('/catalog/items?kind=equipment&limit=1').then((r) => (catalog = r.total)).catch(() => {});
  if (can('parametres', 'admin')) Promise.all([api.get<any>('/admin/update'), api.get<any>('/admin/backups')]).then(([u, b]) => (site = { version: u.current, available: u.status?.available?.version ?? null, lastBackup: b.last_success })).catch(() => {});

  const lead = $derived.by(() => {
    if (!dash) return 'Bienvenue sur Strata Arkyo.';
    if (!dash.projects_total) return 'Aucun projet pour l’instant. Créez le premier : l’assistant vous guide étape par étape.';
    const n = dash.upcoming.length;
    const s = n ? `${n} projet${n > 1 ? 's démarrent' : ' démarre'} dans les 30 prochains jours.` : 'Aucun projet ne démarre dans les 30 prochains jours.';
    return s;
  });
  const statusList = $derived(dash ? STATUS_ORDER.map((s) => [s, dash!.by_status[s] ?? 0] as const).filter(([, n]) => n > 0) : []);
</script>

<div class="page">
  <section class="home-hero">
    <header class="hero">
      <p class="eyebrow">{today}</p>
      <h1 class="display">Bonjour {first}.</h1>
      <p class="lead">{lead}{#if dash?.next}{' '}Le plus proche, <a class="ilink" href="/projets/{dash.next.project.id}">{dash.next.project.name}</a>, démarre {fdate(dash.next.project.shoot_start ?? dash.next.project.commissioning, { weekday: 'long', day: 'numeric', month: 'long' })}.{/if}</p>
      <div class="head-actions">
        {#if can('projets', 'ecriture')}<a class="btn primary" href="/projets?nouveau=1"><Icon name="plus" />Nouveau projet</a>{/if}
        {#if can('projets', 'lecture')}<a class="btn" href="/projets">Tous les projets</a>{/if}
      </div>
    </header>
    {#if dash?.next}
      {@const n = dash.next}
      <a class="next" href="/projets/{n.project.id}">
        <span class="next-top"><span class="lbl-t">Prochaine exploitation · {REGIE[n.project.regie].label}</span><span class="tally">J-{n.days}</span></span>
        <span class="next-vis"><Ill kind={REGIE[n.project.regie].ill} cables pulse w={380} /></span>
        <span class="next-name">{n.project.name}</span>
        <span class="next-sub">{n.project.client ?? 'Projet interne'}{n.project.venue ? ` · ${n.project.venue}` : ''}</span>
        <span class="next-meta">
          <span><span class="lbl-t">Exploitation</span><b>{fdate(n.project.shoot_start ?? n.project.commissioning, { weekday: 'short', day: 'numeric' })} → {fdate(n.project.shoot_end ?? n.project.shoot_start ?? n.project.commissioning, { weekday: 'short', day: 'numeric', month: 'short' })}</b></span>
          <span><span class="lbl-t">Fiche · {n.completion} %</span><Vu value={n.completion} n={24} /></span>
          <span></span>
        </span>
      </a>
    {:else if site}
      <a class="next" href="/admin/mises-a-jour">
        <span class="next-top"><span class="lbl-t">État du site</span></span>
        <span class="next-name">Strata Arkyo {site.version}</span>
        <span class="next-sub">{site.available ? `La version ${site.available} est prête à installer.` : 'Le moteur est à jour.'}</span>
        <span class="next-meta" style="grid-template-columns:1fr"><span><span class="lbl-t">Dernière sauvegarde</span><b>{relative(site.lastBackup)}</b></span></span>
      </a>
    {/if}
  </section>

  {#if dash}
    <section class="figs">
      <a class="fig" href="/projets"><span class="lbl-t">Projets actifs</span><span><b>{dash.projects_active}<small>/{dash.projects_total}</small></b><span class="fig-s">en préparation, prêts ou en cours</span></span></a>
      <div class="fig"><span class="lbl-t">Tâches terminées</span><span><b>{dash.tasks_pct}<small>%</small></b><span class="fig-s">{dash.tasks_done} sur {dash.tasks_total} tâche{dash.tasks_total > 1 ? 's' : ''}</span></span><span class="fig-v"><Vu value={dash.tasks_pct} n={14} cls="acc" /></span></div>
      <div class="fig"><span class="lbl-t">Complétion des fiches</span><span><b>{dash.completion_avg}<small>%</small></b><span class="fig-s">en moyenne sur {dash.projects_total} projet{dash.projects_total > 1 ? 's' : ''}</span></span><span class="fig-v"><Vu value={dash.completion_avg} n={14} cls={dash.completion_avg >= 80 ? 'ok' : ''} /></span></div>
      {#if catalog != null}<a class="fig" href="/catalogue"><span class="lbl-t">Catalogue</span><span><b>{nf(catalog)}</b><span class="fig-s">fiches d’équipement</span></span></a>{/if}
    </section>

    <div class="cols">
      <section class="card">
        <div class="card-head"><h2>À venir</h2><span class="muted small">30 prochains jours</span></div>
        <div class="rows">
          {#each dash.upcoming as u (u.project.id)}
            {@const dt = new Date(u.date + 'T12:00:00')}
            <a class="urow" href="/projets/{u.project.id}">
              <span class="r-date"><b>{dt.getDate()}</b><span>{dt.toLocaleDateString('fr-FR', { month: 'short' })}</span></span>
              <span class="r-main"><b>{u.project.name}</b><span>{u.project.client ?? 'Projet interne'}{u.project.venue ? ` · ${u.project.venue}` : ''} · fiche {u.completion} %</span></span>
              <span class="r-j" style="color:{urgency(u.days)}">J-{u.days}</span>
            </a>
          {:else}<p class="empty">Rien de prévu dans les 30 prochains jours.</p>{/each}
        </div>
      </section>
      <section class="card">
        <div class="card-head"><h2>Activité récente</h2></div>
        <div class="rows">
          {#each dash.recent as r (r.project.id)}
            <a class="urow" href="/projets/{r.project.id}" style="grid-template-columns:auto minmax(0,1fr) auto">
              <Avatar name={r.project.name} color={STATUS[r.project.status].color} size="sm" />
              <span class="r-main"><b>{r.project.name}</b><span>modifié {relative(r.project.updated_at)}</span></span>
              <span class="chip st" style="--c:{STATUS[r.project.status].color};font-size:13px"><i class="dot"></i>{STATUS[r.project.status].label}</span>
            </a>
          {:else}<p class="empty">Aucune activité.</p>{/each}
        </div>
      </section>
    </div>

    {#if statusList.length}
      <section class="card">
        <div class="card-head"><h2>Projets par statut</h2><span class="muted small">{dash.projects_total} projets</span></div>
        <div class="stackbar">{#each statusList as [s, n]}<i style="--c:{STATUS[s].color};flex:{n}" title="{STATUS[s].label} : {n}"></i>{/each}</div>
        <div class="legend">{#each statusList as [s, n]}<div><span class="sw" style="--c:{STATUS[s].color}"></span>{STATUS[s].label}<b>{n} · {Math.round((n * 100) / dash.projects_total)} %</b></div>{/each}</div>
      </section>
    {/if}
  {/if}
</div>
