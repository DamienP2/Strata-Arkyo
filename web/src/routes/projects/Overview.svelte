<script lang="ts">
  // Vue d'ensemble d'un projet : bandeau (nom, type, compte à rebours, frise des périodes),
  // anneau de préparation et domaines, puis deux colonnes : « À faire » et informations à gauche,
  // inventaire, tâches, équipe, activité et projets liés à droite.
  import { tick, untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Avatar from '../../components/Avatar.svelte';
  import Menu from '../../components/Menu.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import KindIll from '../../components/KindIll.svelte';
  import Loading from '../../components/Loading.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import ProjectLinks from '../../components/ProjectLinks.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import { groupAnomalies, type Anomaly, type AnomalyGroup } from '../../lib/anomalies';
  import { api, ApiError } from '../../lib/api';
  import { can, session } from '../../lib/session.svelte';
  import { live } from '../../lib/live.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { navigate, registerLeaveGuard } from '../../lib/router.svelte';
  import { ask } from '../../lib/ask.svelte';
  import { nf, dateTime } from '../../lib/format';
  import type { Member } from '../../lib/types';
  import { PROJECT_TYPES, CONTAINER_KINDS, TASK_STATUS, projectType, daysBetween, fdate, urgency, type Project, type ProjectView, type Task, type Container } from '../../lib/projects';
  import { addDays, isOpen, taskBody } from './tasks';

  let { view = $bindable(), reload }: { view: ProjectView; reload: () => Promise<void> } = $props();
  const writable = $derived(can('projets', 'ecriture'));
  const p = $derived(view.project);
  const pid = $derived(view.project.id);
  const today = $derived(view.today);
  const fixe = $derived(projectType(p.regie) === 'fixe');
  const explLabel = $derived(fixe ? 'Mise en service' : 'Exploitation');
  const explWhen = $derived(fixe ? 'avant la mise en service' : 'avant l’exploitation');

  // ----- Données lues à côté de la vue : avancement, journal, membres -----
  interface Item { kind: string; group?: string; severity: string; subject?: string; place?: string | null; detail?: string; message: string; href: string }
  interface Domain { key: string; label: string; done: number; total: number; pct: number; href: string; note: string; hint?: string }
  interface Todo { domains: Domain[]; items: Item[]; pct?: number }
  interface Entry { id: number; at: string; actor_id: string | null; action_label: string; object_label: string; subject: string; detail: string }
  let todo = $state<Todo | null>(null);
  let entries = $state<Entry[] | null>(null);
  let names = $state<Record<string, string>>({});
  // Relus à chaque changement du projet (la vue se recharge sur les modifications des autres).
  $effect(() => {
    void view; void view.project;
    const id = untrack(() => view.project.id);
    api.get<Todo>(`/projects/${id}/todo`).then((t) => (todo = t)).catch(() => {});
    api.get<{ entries: Entry[] }>(`/projects/${id}/history?limit=40`).then((r) => (entries = r.entries)).catch(() => (entries ??= []));
  });
  api.get<{ members: Member[] }>('/members?removed=true').then((m) => (names = Object.fromEntries(m.members.map((x) => [x.id, x.name])))).catch(() => {});

  // ----- Présence -----
  const meId = $derived(session.me?.account.id ?? '');
  const presence = $derived(live.presence[`project:${pid}`] ?? view.presence ?? []);
  // Les autres membres présents (un même compte ouvert dans plusieurs onglets ne compte qu'une fois).
  const others = $derived([...new Map(presence.filter((m) => m.id !== meId).map((m) => [m.id, m])).values()]);
  const presenceHelp = $derived(`Aussi sur ce projet : ${others.map((m) => m.name).join(', ')}. Leurs modifications s’affichent sans recharger la page.`);

  // ----- Dates : compte à rebours et frise -----
  const explStart = $derived(p.shoot_start ?? p.commissioning);
  const explEnd = $derived(p.shoot_end ?? explStart);
  const countdown = $derived.by(() => {
    if (!explStart || !explEnd) return null;
    if (today < explStart) { const n = daysBetween(today, explStart); return { kind: 'before' as const, n, label: explWhen }; }
    if (today <= explEnd) return { kind: 'now' as const, n: 0, label: fixe ? 'Mise en service' : 'Exploitation' };
    const n = daysBetween(explEnd, today);
    return { kind: 'after' as const, n, label: `il y a ${n} j` };
  });
  /** Période lisible : « 16 → 21 nov. », « 28 nov. → 4 déc. », un seul jour si identiques. */
  function span(a: string, b: string): string {
    if (a === b) return fdate(a);
    const sameMonth = a.slice(0, 7) === b.slice(0, 7);
    return `${sameMonth ? fdate(a, { day: 'numeric' }) : fdate(a)} → ${fdate(b)}`;
  }
  const minD = (xs: string[]) => xs.reduce((m, x) => (x < m ? x : m));
  const maxD = (xs: string[]) => xs.reduce((m, x) => (x > m ? x : m));
  interface Seg { key: 'prep' | 'expl'; label: string; a: string; b: string }
  const segs = $derived.by(() => {
    const out: Seg[] = [];
    const ps = p.planning_start ?? p.planning_end, pe = p.planning_end ?? p.planning_start;
    if (ps && pe) out.push({ key: 'prep', label: 'Préparation', a: minD([ps, pe]), b: maxD([ps, pe]) });
    if (explStart && explEnd) out.push({ key: 'expl', label: explLabel, a: minD([explStart, explEnd]), b: maxD([explStart, explEnd]) });
    return out;
  });
  // Plage : des premières aux dernières dates, élargie de quelques jours de chaque côté.
  const tl = $derived.by(() => {
    if (!segs.length) return null;
    const all = segs.flatMap((s) => [s.a, s.b]);
    const pad = Math.max(2, Math.round((daysBetween(minD(all), maxD(all)) + 1) * 0.08));
    const lo = addDays(minD(all), -pad), hi = addDays(maxD(all), pad);
    const total = daysBetween(lo, hi) + 1;
    const at = (d: string) => (daysBetween(lo, d) / total) * 100;
    const ticks = Array.from({ length: 5 }, (_, k) => {
      const d = addDays(lo, Math.round((k * (total - 1)) / 4));
      return { d, x: ((daysBetween(lo, d) + 0.5) / total) * 100, k };
    });
    const now = today >= lo && today <= hi ? at(today) + 50 / total : null;
    const placed = segs.map((s) => {
      const x = at(s.a), w = ((daysBetween(s.a, s.b) + 1) / total) * 100;
      // Période trop courte pour son libellé : le libellé passe à côté, du côté le plus dégagé.
      return { ...s, x, w, narrow: w < 16, after: x + w <= 60 };
    });
    // Périodes qui se chevauchent ou libellé à côté : une ligne par période.
    const two = placed.length > 1 && (placed.some((s) => s.narrow) || placed[1]!.a <= placed[0]!.b);
    return { segs: placed.map((s, i) => ({ ...s, lane: two ? i : 0 })), two, ticks, now };
  });

  // ----- Préparation -----
  // Moyenne des domaines qui ont quelque chose à compter (un domaine vide n'est pas « prêt à 100 % »).
  const counted = $derived((todo?.domains ?? []).filter((d) => d.total > 0));
  const ready = $derived(todo?.pct ?? (counted.length ? Math.round(counted.reduce((s, d) => s + d.pct, 0) / counted.length) : 0));
  const RING = 2 * Math.PI * 50;
  const DONE: Record<string, string> = {
    material: 'tout est rangé dans un élément', racks: 'tout est posé en rack', cabling: 'tous les câbles sont numérotés',
    validation: 'tous les points sont testés', sheet: 'informations complètes',
  };
  /** Domaine sans rien à compter : ce qui manque pour qu'il le devienne. */
  const EMPTY: Record<string, string> = {
    material: 'Aucun équipement dans l’inventaire', racks: 'Aucun équipement rackable dans un rack',
    cabling: 'Aucun câble dans le synoptique', validation: 'Aucun point de recette',
  };
  const doneDomains =$derived((todo?.domains ?? []).filter((d) => d.pct === 100 && d.total > 0));

  // ----- À faire : un point par type d'anomalie, trié par gravité -----
  const KIND_GROUP: Record<string, string> = { network: 'Réseau', dmx: 'DMX', sheet: 'Informations', rack: 'Non posé en rack' };
  const anomalies = $derived<Anomaly<Item>[]>((todo?.items ?? []).map((it) => ({
    group: it.group ?? KIND_GROUP[it.kind] ?? it.kind, severity: it.severity,
    subject: it.subject ?? it.message, place: it.place, detail: it.subject ? (it.detail ?? '') : '', href: it.href, src: it,
  })));
  const groups = $derived(groupAnomalies(anomalies));
  /** Titre d'un type d'anomalie, au singulier et au pluriel. */
  const TITLES: Record<string, [string, string]> = {
    'Extrémité manquante': ['Câble avec une extrémité manquante', 'Câbles avec une extrémité manquante'],
    'Numéro en double': ['Numéro en double', 'Numéros en double'],
    'Port surutilisé': ['Port qui porte plusieurs câbles', 'Ports qui portent plusieurs câbles'],
    'Incompatibilité': ['Câble entre ports incompatibles', 'Câbles entre ports incompatibles'],
    'Attache incompatible': ['Attache d’adaptateur incompatible', 'Attaches d’adaptateur incompatibles'],
    'Câble sans numéro': ['Câble sans numéro', 'Câbles sans numéro'],
    'Emplacement vide': ['Emplacement vide', 'Emplacements vides'],
    'Équipement non câblé': ['Équipement non câblé', 'Équipements non câblés'],
    'Panel non câblé': ['Panel non câblé', 'Panels non câblés'],
    'Panel mal câblé': ['Panel câblé d’un seul côté', 'Panels câblés d’un seul côté'],
    'Adaptateur orphelin': ['Adaptateur rattaché à rien', 'Adaptateurs rattachés à rien'],
    'Zone sans nom': ['Zone sans nom', 'Zones sans nom'],
    'Zone vide': ['Zone vide', 'Zones vides'],
    'Non posé en rack': ['Exemplaire pas encore posé en rack', 'Exemplaires pas encore posés en rack'],
    'À vérifier': ['Câble à vérifier', 'Câbles à vérifier'],
    'Réseau': ['Point réseau à corriger', 'Points réseau à corriger'],
    'DMX': ['Conflit d’adresses DMX', 'Conflits d’adresses DMX'],
    'Informations': ['Informations à compléter', 'Informations à compléter'],
  };
  const SCREENS: [string, string][] = [['/cables', 'Liste des câbles'], ['/synoptique', 'Synoptique'], ['/racks', 'Racks'], ['/reseau', 'Réseau'], ['/dessins', 'Dessins'], ['/inventaire', 'Inventaire'], ['/recette', 'Recette']];
  const screenOf = (href: string) => SCREENS.find(([k]) => href.includes(k))?.[1] ?? '';
  const SEV: Record<string, string> = { error: 'err', warning: 'warn', info: 'info' };
  /** Où agir pour un type : la liste des câbles filtrée sur eux, l'objet commun, sinon l'écran. */
  function groupHref(g: AnomalyGroup<Item>): string {
    const ids = g.items.flatMap((a) => new URLSearchParams(a.href?.split('?')[1] ?? '').get('cable')?.split(',') ?? []);
    if (ids.length) return `/projets/${pid}/cables?cable=${[...new Set(ids)].join(',')}`;
    const hrefs = [...new Set(g.items.map((a) => a.href ?? ''))];
    if (hrefs.length === 1) return hrefs[0]!;
    const m = /^(\/projets\/[^/]+\/[^/?]+)/.exec(hrefs[0] ?? '');
    return m ? m[1]! : `/projets/${pid}`;
  }
  function groupSub(g: AnomalyGroup<Item>): string {
    if (g.key === 'Informations') return (g.items[0]?.src?.message ?? '').replace(/^[^:]*:\s*/, '').replace(/\.$/, '');
    const subs = [...new Set(g.items.map((a) => a.subject))];
    const head = subs.slice(0, 2).join(', ') + (subs.length > 2 ? ` et ${subs.length - 2} autre${subs.length > 3 ? 's' : ''}` : '');
    return head;
  }
  const todoRows = $derived(groups.map((g) => ({
    key: g.key, sev: SEV[g.severity] ?? 'info', n: g.items.length,
    title: TITLES[g.key]?.[g.items.length > 1 ? 1 : 0] ?? g.key, sub: groupSub(g), where: g.key === 'Informations' ? '' : screenOf(g.items[0]?.href ?? ''),
    href: g.key === 'Informations' ? null : groupHref(g),
  })));
  let allTodo = $state(false);
  const TODO_MAX = 6;
  const shownTodo = $derived(allTodo ? todoRows : todoRows.slice(0, TODO_MAX));

  // ----- Inventaire -----
  const pieces = $derived(view.articles.reduce((s, a) => s + a.quantity, 0));
  const weight = $derived(view.locations.reduce((s, l) => s + l.totals.weight_kg, 0));
  const power = $derived(view.locations.reduce((s, l) => s + l.totals.power_w, 0));
  const locPos = $derived(Object.fromEntries(view.locations.map((l) => [l.id, l.position])));
  const SIL_MAX = 8;
  const elements = $derived([...view.containers].sort((a, b) => (locPos[a.location_id] ?? 0) - (locPos[b.location_id] ?? 0) || a.position - b.position));
  const fu = (u: number) => nf(u, u % 1 ? 1 : 0);
  /** Silhouette d'un élément rackable : cases de hauteur, allumées selon les U posés. */
  function rackSil(c: Container) {
    const H = c.height_u ?? 0, placed = c.totals.placed_u ?? c.totals.used_u;
    const n = Math.max(3, Math.min(14, Math.round(H / 3)));
    const lit = H ? Math.min(n, Math.round((placed / H) * n)) : 0;
    return { n, lit, h: Math.round(Math.max(40, Math.min(96, 30 + H * 1.6))), placed, H };
  }

  // ----- Tâches -----
  const doneTasks = $derived(view.tasks.filter((t) => !isOpen(t)).length);
  const nextTasks = $derived(view.tasks.filter(isOpen).sort((a, b) => (a.due === b.due ? a.position - b.position : !a.due ? 1 : !b.due ? -1 : a.due.localeCompare(b.due))).slice(0, 4));
  function dueText(d: string): string {
    const n = daysBetween(today, d);
    return n === 0 ? 'aujourd’hui' : n === 1 ? 'demain' : n === -1 ? 'hier' : fdate(d);
  }
  let ticking = $state<string | null>(null);
  // Case cochée : la tâche passe à « Terminé », annulable depuis la notification.
  async function finish(t: Task) {
    ticking = t.id;
    const before = taskBody(t);
    try {
      const after = await api.patch<Task>(`/tasks/${t.id}`, { ...before, status: 'done', rev: t.rev });
      const i = view.tasks.findIndex((x) => x.id === t.id);
      if (i >= 0) view.tasks[i] = after;
      toast(`Tâche « ${t.title} » passée à « ${TASK_STATUS.done.label} ».`, 'ok', { run: async () => {
        const cur = view.tasks.find((x) => x.id === t.id) ?? after;
        try { await api.patch(`/tasks/${t.id}`, { ...before, rev: cur.rev }); await reload(); } catch (e) { fail(e); await reload(); return false; }
      } });
      await reload();
    } catch (e) { fail(e); await reload(); } finally { ticking = null; }
  }

  // ----- Équipe et activité -----
  const nameOf = (id: string | null) => (id ? (id === meId ? (session.me?.account.name ?? 'Vous') : names[id] ?? presence.find((m) => m.id === id)?.name ?? 'Compte supprimé') : 'Système');
  const colorOf = (id: string | null) => presence.find((m) => m.id === id)?.color;
  const online = $derived(new Set(presence.map((m) => m.id)));
  const team = $derived.by(() => {
    const out = new Map<string, { id: string; name: string; color?: string; on: boolean }>();
    for (const m of presence) out.set(m.id, { id: m.id, name: m.id === meId ? nameOf(m.id) : m.name, color: m.color, on: true });
    for (const e of entries ?? []) if (e.actor_id && !out.has(e.actor_id) && (names[e.actor_id] || e.actor_id === meId)) out.set(e.actor_id, { id: e.actor_id, name: nameOf(e.actor_id), on: false });
    return [...out.values()];
  });
  const TEAM_MAX = 6;
  const recent = $derived((entries ?? []).slice(0, 5));
  function ago(iso: string): string {
    const s = (Date.now() - new Date(iso).getTime()) / 1000;
    if (s < 60) return 'à l’instant';
    if (s < 3600) return `${Math.round(s / 60)} min`;
    if (s < 86400) return `${Math.round(s / 3600)} h`;
    if (s < 86400 * 2) return 'hier';
    if (s < 86400 * 7) return `${Math.round(s / 86400)} j`;
    return fdate(iso.slice(0, 10));
  }

  // ----- Informations : fiche en lecture, un champ ou tout en édition -----
  type Row = 'name' | 'client' | 'venue' | 'prep' | 'expl' | 'description';
  const ROW_FIELDS: Record<Row, string[]> = {
    name: ['name'], client: ['client'], venue: ['venue'], prep: ['planning_start', 'planning_end'],
    expl: ['shoot_start', 'shoot_end', 'commissioning'], description: ['description'],
  };
  let form = $state<Project | null>(null);
  let base = '';
  let editing = $state<Row | null>(null);
  let dialog = $state(false);
  let errors = $state<Record<string, string>>({});
  let busy = $state(false);
  const dirty = $derived(!!form && JSON.stringify(form) !== base);
  // Saisie commencée et non enregistrée : on demande avant de quitter l'écran ou l'onglet.
  $effect(() => registerLeaveGuard(() => dirty));
  function begin() {
    const f = structuredClone($state.snapshot(view.project)) as Project;
    // Ancienne date de mise en service : reprise dans la période (le serveur fait de même).
    if (!f.shoot_start && f.commissioning) { f.shoot_start = f.commissioning; f.shoot_end ??= f.commissioning; }
    f.commissioning = null;
    form = f; base = JSON.stringify(f); errors = {};
  }
  // Une mise à jour venue d'un autre membre remplace la saisie si elle n'a pas été touchée.
  $effect(() => {
    void view.project;
    untrack(() => { if (form && !dirty) begin(); });
  });
  async function edit(r: Row) {
    if (!writable) return;
    if (!form || editing !== r) begin();
    editing = r; dialog = false;
    await tick();
    document.querySelector<HTMLElement>(`#ov-in-${r}`)?.focus();
  }
  async function stopEdit(r: Row | null) {
    form = null; editing = null; errors = {};
    if (r) { await tick(); document.querySelector<HTMLElement>(`#ov-pen-${r}`)?.focus(); }
  }
  function openAll() { if (!writable) return; begin(); editing = null; dialog = true; }
  const txt = (k: keyof Project) => (e: Event) => { if (form) (form as any)[k] = (e.target as HTMLInputElement).value || null; };
  /** Mise en service : un seul jour, la fin suit le début. */
  const commission = (e: Event) => { if (form) { const v = (e.target as HTMLInputElement).value || null; form.shoot_start = v; form.shoot_end = v; } };
  /** Champ en erreur : aria-invalid et aria-describedby vers le message (FieldError `for`). */
  const inv = (key: string, id: string) => ({ 'aria-invalid': errors[key] ? true : undefined, 'aria-describedby': errors[key] ? `${id}-err` : undefined });
  async function save(): Promise<void> {
    if (!form) return;
    const row = editing;
    if (!dirty) { if (dialog) { dialog = false; form = null; } else await stopEdit(row); return; }
    busy = true; errors = {};
    try {
      const r = await api.patch<Project>(`/projects/${form.id}`, { ...$state.snapshot(form), commissioning: null });
      view.project = r; dialog = false;
      toast('Informations enregistrées.');
      await stopEdit(row);
    } catch (x) {
      if (x instanceof ApiError && x.code === 'stale') {
        toast('Un autre membre a modifié les informations du projet : elles sont rechargées.', 'warn');
        await reload(); form = null; editing = null; dialog = false;
      } else if (x instanceof ApiError && x.fields.length) {
        errors = Object.fromEntries(x.fields.map((f) => [f.field === 'commissioning' ? 'shoot_start' : fixe && f.field === 'shoot_end' ? 'shoot_start' : f.field, f.message]));
        // En ligne, une erreur sur un autre champ que celui modifié s'annonce en notification.
        if (row) { const away = x.fields.filter((f) => !ROW_FIELDS[row].includes(f.field)); if (away.length) toast(away.map((f) => f.message).join(' '), 'warn'); }
      } else fail(x);
    } finally { busy = false; }
  }
  function inlineKey(e: KeyboardEvent) {
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); stopEdit(editing); }
    else if (e.key === 'Enter' && e.target instanceof HTMLTextAreaElement && !e.shiftKey) { e.preventDefault(); save(); }
  }
  /** Ligne en édition : Échap annule, Entrée enregistre (Maj+Entrée va à la ligne dans la description). */
  function inlineKeys(node: HTMLElement) {
    node.addEventListener('keydown', inlineKey);
    return { destroy: () => node.removeEventListener('keydown', inlineKey) };
  }

  // Suppression en cascade : confirmation, la requête part depuis la boîte (erreur affichée dedans).
  async function remove() {
    const { id, name } = view.project;
    const ok = await ask({
      title: `Supprimer « ${name} » ?`, eyebrow: 'Projet', confirmLabel: 'Supprimer définitivement', danger: true,
      text: 'Les emplacements, éléments, matériel, câbles et tâches partent avec lui. Il reste récupérable pendant un temps depuis Administration › Sauvegardes (projets supprimés), puis seulement depuis une sauvegarde du site.',
      run: () => api.del(`/projects/${id}`),
    });
    if (!ok) return;
    form = null; // la saisie en cours part avec le projet : pas de garde de sortie
    toast(`« ${name} » supprimé.`);
    navigate('/projets');
  }
  async function addDescription() {
    document.getElementById('ov-info')?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    await edit('description');
  }
</script>

<!-- Bouton crayon d'une ligne de la fiche (visible au survol, toujours au clavier et au doigt). -->
{#snippet pen(r: Row, label: string, empty: boolean)}
  {#if writable}<button type="button" class="icon-btn sm ov-pen" id="ov-pen-{r}" aria-label={label} title={label} onclick={() => edit(r)}><Icon name={empty ? 'plus' : 'pen'} size="sm" /></button>{/if}
{/snippet}
<!-- Boutons d'une ligne en édition : Entrée enregistre, Échap annule. -->
{#snippet inlineActs(r: Row)}
  <span class="ov-inl-acts">
    <button type="button" class="icon-btn sm" aria-label="Annuler (Échap)" title="Annuler (Échap)" disabled={busy} onclick={() => stopEdit(r)}><Icon name="x" size="sm" /></button>
    <button type="submit" class="icon-btn sm ov-ok" aria-label="Enregistrer (Entrée)" title="Enregistrer (Entrée)" disabled={busy}>{#if busy}<span class="spin" aria-hidden="true"></span>{:else}<Icon name="ok" size="sm" />{/if}</button>
  </span>
{/snippet}

<div class="ov">
  <!-- Bandeau -->
  <section class="ov-hero" aria-label="Projet">
    <div class="ov-id">
      <div class="ov-tags">
        <span class="chip">{PROJECT_TYPES[projectType(p.regie)].label}</span>
        {#if others.length}<span class="chip ok" title={presenceHelp}><i class="dot"></i>{others.length + 1} personnes connectées<span class="sr-only"> : {presenceHelp}</span></span>{/if}
      </div>
      <h2 class="ov-name">{p.name}</h2>
      <div class="ov-meta">
        <span><Icon name="users" size="sm" />{p.client ?? 'Projet interne'}</span>
        {#if p.venue}<span><Icon name="pin" size="sm" />{p.venue}</span>{/if}
        {#if !p.description && writable}<button type="button" class="ov-add" onclick={addDescription}><Icon name="plus" size="sm" />Ajouter une description</button>{/if}
      </div>
      {#if p.description}<p class="ov-desc" title={p.description}>{p.description}</p>{/if}
    </div>
    <div class="ov-count">
      {#if countdown}
        {#if countdown.kind === 'before'}
          <b class="ov-big" style="--c:{countdown.n <= 14 ? urgency(countdown.n) : 'var(--fg)'}" title="{countdown.n} jour{countdown.n > 1 ? 's' : ''} {countdown.label}"><small>J-</small>{countdown.n}</b>
        {:else if countdown.kind === 'now'}<b class="ov-big ov-word" style="--c:var(--st-cours)">En cours</b>
        {:else}<b class="ov-big ov-word" style="--c:var(--fg-3)">Terminé</b>{/if}
        <span class="ov-lbl">{countdown.label}</span>
      {/if}
      <div class="ov-acts">
        {#if writable}<button type="button" class="btn sm" onclick={openAll}><Icon name="pen" size="sm" />Modifier</button>{/if}
        <Menu label="Plus d’actions" icon="more" iconOnly btnClass="icon-btn sm">
          {#snippet children()}
            <a role="menuitem" href="/projets/{pid}/historique"><Icon name="clock" size="sm" />Historique</a>
            <a role="menuitem" href="/projets/{pid}/dossier"><Icon name="file" size="sm" />Dossier et exports</a>
            {#if writable}<hr><button type="button" role="menuitem" class="danger" onclick={remove}><Icon name="trash" size="sm" />Supprimer le projet…</button>{/if}
          {/snippet}
        </Menu>
      </div>
    </div>
    {#if tl}
      <div class="ov-tl">
        <div class="ov-track" class:two={tl.two} role="group" aria-label="Calendrier du projet">
          {#each tl.segs as s (s.key)}
            {#if s.narrow}
              <div class="ov-seg {s.key} l{s.lane}" style="left:{s.x}%;width:{s.w}%" title="{s.label} : {span(s.a, s.b)}"></div>
              <div class="ov-seg-out l{s.lane}" style={s.after ? `left:calc(${s.x + s.w}% + 8px)` : `right:calc(${100 - s.x}% + 8px)`}><b>{s.label}</b><span class="mono">{span(s.a, s.b)}</span></div>
            {:else}
              <div class="ov-seg {s.key} l{s.lane}" style="left:{s.x}%;width:{s.w}%" title="{s.label} : {span(s.a, s.b)}"><b>{s.label}</b><span class="mono">{span(s.a, s.b)}</span></div>
            {/if}
          {/each}
          {#if tl.now != null}<div class="ov-now" class:l={tl.now < 10} class:r={tl.now > 90} style="left:{tl.now}%"><span>Aujourd’hui</span></div>{/if}
        </div>
        <div class="ov-scale" aria-hidden="true">
          {#each tl.ticks as t (t.k)}<span class="t{t.k}" style="left:{t.x}%">{fdate(t.d)}</span>{/each}
        </div>
      </div>
    {/if}
  </section>

  <!-- Préparation -->
  <section class="card ov-ready" aria-label="Préparation du projet">
    {#if !todo}<Loading label="Lecture de l’avancement" />
    {:else}
      <div class="ov-ring" role="img" aria-label="Préparation globale : {ready} %">
        <svg viewBox="0 0 120 120" aria-hidden="true"><circle cx="60" cy="60" r="50" class="trk" /><circle cx="60" cy="60" r="50" class="val" class:ok={ready === 100} stroke-dasharray={RING} stroke-dashoffset={RING * (1 - ready / 100)} /></svg>
        <div class="ov-ring-v"><b>{ready} %</b><span>prêt</span></div>
      </div>
      <div class="ov-doms">
        {#each todo.domains as d (d.key)}
          {#if d.key === 'sheet'}
            <button type="button" class="ov-dom" title={d.hint ? `${d.note}. ${d.hint}` : d.note} onclick={() => document.getElementById('ov-info')?.scrollIntoView({ behavior: 'smooth', block: 'center' })}>
              <span class="t">{d.label}<Icon name="chevr" size="sm" /></span><b>{d.total ? `${d.pct} %` : "—"}</b><span class="bar"><i class:ok={d.total > 0 && d.pct === 100} style="width:{d.total ? d.pct : 0}%"></i></span><span class="n">{d.total ? d.note : (EMPTY[d.key] ?? d.note)}</span>
            </button>
          {:else}
            <a class="ov-dom" href={d.href} title={d.hint ? `${d.note}. ${d.hint}` : d.note}>
              <span class="t">{d.label}<Icon name="chevr" size="sm" /></span><b>{d.total ? `${d.pct} %` : "—"}</b><span class="bar"><i class:ok={d.total > 0 && d.pct === 100} style="width:{d.total ? d.pct : 0}%"></i></span><span class="n">{d.total ? d.note : (EMPTY[d.key] ?? d.note)}</span>
            </a>
          {/if}
        {/each}
      </div>
    {/if}
  </section>

  <div class="ov-cols">
    <div class="ov-side">
      <!-- À faire -->
      <section class="card ov-todo">
        <div class="card-head"><h2>À faire {explWhen}</h2>{#if todo && todoRows.length}<span class="muted small mono">{todo.items.length} point{todo.items.length > 1 ? 's' : ''}</span>{/if}</div>
        {#if !todo}<Loading label="Lecture des points à traiter" />
        {:else if !todoRows.length}<EmptyState card={false} icon="ok" message="Rien en attente" hint="Aucune anomalie, tout est posé et les informations du projet sont complètes." />
        {:else}
          <ul class="ov-todo-l">
            {#each shownTodo as r (r.key)}
              <li>
                {#snippet rowBody()}<span class="sev {r.sev}"></span><span class="tx"><span class="tt">{r.title}</span><span class="ss"><span class="ss-h">{r.sub}</span>{#if r.where}<span class="ss-w">{"\u00a0· "}{r.where}</span>{/if}</span></span><span class="num mono">{r.n}</span><Icon name="chevr" size="sm" />{/snippet}
                {#if r.href}<a class="ov-todo-it" href={r.href}>{@render rowBody()}</a>
                {:else}<button type="button" class="ov-todo-it" onclick={() => (writable ? openAll() : document.getElementById('ov-info')?.scrollIntoView({ behavior: 'smooth', block: 'center' }))}>{@render rowBody()}</button>{/if}
              </li>
            {/each}
          </ul>
          {#if todoRows.length > TODO_MAX}<div class="ov-more"><button type="button" class="btn ghost sm" onclick={() => (allTodo = !allTodo)}>{allTodo ? 'Afficher moins' : `Afficher tout (${TODO_MAX} sur ${todoRows.length})`}</button></div>{/if}
        {/if}
        {#if todo && doneDomains.length}
          <div class="ov-done">
            {#each doneDomains as d (d.key)}<span><Icon name="ok" size="sm" />{d.label} : {DONE[d.key] ?? d.note}.</span>{/each}
          </div>
        {/if}
      </section>

      <!-- Informations -->
      <section class="card ov-info" id="ov-info">
        <div class="card-head"><h2>Informations</h2>{#if writable}<button type="button" class="btn ghost sm" onclick={openAll}>Tout modifier</button>{/if}</div>
        <dl class="ov-dl">
          <div class="ov-row" class:on={editing === 'name'}>
            <dt>Nom</dt>
            <dd>
              {#if editing === 'name' && form}
                <form class="ov-inl" onsubmit={(e) => { e.preventDefault(); save(); }} use:inlineKeys>
                  <span class="ov-inl-f"><input class="input sm" id="ov-in-name" aria-label="Nom" bind:value={form.name} required maxlength="120" {...inv('name', 'ov-in-name')}><FieldError message={errors.name} for="ov-in-name" /></span>
                  {@render inlineActs('name')}
                </form>
              {:else}<span class="v">{p.name}</span>{@render pen('name', 'Modifier le nom', false)}{/if}
            </dd>
          </div>
          <div class="ov-row" class:on={editing === 'client'}>
            <dt>Client</dt>
            <dd>
              {#if editing === 'client' && form}
                <form class="ov-inl" onsubmit={(e) => { e.preventDefault(); save(); }} use:inlineKeys>
                  <span class="ov-inl-f"><input class="input sm" id="ov-in-client" aria-label="Client" value={form.client ?? ''} oninput={txt('client')} placeholder="Vide : projet interne" maxlength="120" {...inv('client', 'ov-in-client')}><FieldError message={errors.client} for="ov-in-client" /></span>
                  {@render inlineActs('client')}
                </form>
              {:else}<span class="v" class:empty={!p.client}>{p.client ?? 'Projet interne'}</span>{@render pen('client', 'Modifier le client', false)}{/if}
            </dd>
          </div>
          <div class="ov-row" class:on={editing === 'venue'}>
            <dt>Lieu</dt>
            <dd>
              {#if editing === 'venue' && form}
                <form class="ov-inl" onsubmit={(e) => { e.preventDefault(); save(); }} use:inlineKeys>
                  <span class="ov-inl-f"><input class="input sm" id="ov-in-venue" aria-label="Lieu" value={form.venue ?? ''} oninput={txt('venue')} maxlength="200" {...inv('venue', 'ov-in-venue')}><FieldError message={errors.venue} for="ov-in-venue" /></span>
                  {@render inlineActs('venue')}
                </form>
              {:else}<span class="v" class:empty={!p.venue}>{p.venue ?? 'Aucun lieu'}</span>{@render pen('venue', p.venue ? 'Modifier le lieu' : 'Ajouter un lieu', !p.venue)}{/if}
            </dd>
          </div>
          <div class="ov-row" class:on={editing === 'prep'}>
            <dt>Préparation</dt>
            <dd>
              {#if editing === 'prep' && form}
                <form class="ov-inl" onsubmit={(e) => { e.preventDefault(); save(); }} use:inlineKeys>
                  <span class="ov-inl-f ov-dates">
                    <input class="input sm" type="date" id="ov-in-prep" aria-label="Début de la préparation" value={form.planning_start ?? ''} oninput={txt('planning_start')} {...inv('planning_start', 'ov-in-prep')}>
                    <input class="input sm" type="date" id="ov-in-prep-e" aria-label="Fin de la préparation" value={form.planning_end ?? ''} oninput={txt('planning_end')} {...inv('planning_end', 'ov-in-prep-e')}>
                    <FieldError message={errors.planning_start} for="ov-in-prep" /><FieldError message={errors.planning_end} for="ov-in-prep-e" />
                  </span>
                  {@render inlineActs('prep')}
                </form>
              {:else}
                {@const a = p.planning_start ?? p.planning_end}
                {@const b = p.planning_end ?? p.planning_start}
                <span class="v" class:mono={!!a} class:empty={!a}>{a && b ? span(a, b) : 'Dates à fixer'}</span>{@render pen('prep', 'Modifier la préparation', !a)}
              {/if}
            </dd>
          </div>
          <div class="ov-row" class:on={editing === 'expl'}>
            <dt>{explLabel}</dt>
            <dd>
              {#if editing === 'expl' && form}
                <form class="ov-inl" onsubmit={(e) => { e.preventDefault(); save(); }} use:inlineKeys>
                  {#if fixe}
                    <span class="ov-inl-f"><input class="input sm" type="date" id="ov-in-expl" aria-label="Date de mise en service" value={form.shoot_start ?? ''} oninput={commission} {...inv('shoot_start', 'ov-in-expl')}><FieldError message={errors.shoot_start} for="ov-in-expl" /></span>
                  {:else}
                    <span class="ov-inl-f ov-dates">
                      <input class="input sm" type="date" id="ov-in-expl" aria-label="Début de l’exploitation" value={form.shoot_start ?? ''} oninput={txt('shoot_start')} {...inv('shoot_start', 'ov-in-expl')}>
                      <input class="input sm" type="date" id="ov-in-expl-e" aria-label="Fin de l’exploitation" value={form.shoot_end ?? ''} oninput={txt('shoot_end')} {...inv('shoot_end', 'ov-in-expl-e')}>
                      <FieldError message={errors.shoot_start} for="ov-in-expl" /><FieldError message={errors.shoot_end} for="ov-in-expl-e" />
                    </span>
                  {/if}
                  {@render inlineActs('expl')}
                </form>
              {:else}
                <span class="v" class:mono={!!explStart} class:empty={!explStart}>{explStart && explEnd ? span(explStart, explEnd) : 'Date à fixer'}</span>{@render pen('expl', fixe ? 'Modifier la mise en service' : 'Modifier l’exploitation', !explStart)}
              {/if}
            </dd>
          </div>
          <div class="ov-row" class:on={editing === 'description'}>
            <dt>Description</dt>
            <dd>
              {#if editing === 'description' && form}
                <form class="ov-inl ov-inl-ta" onsubmit={(e) => { e.preventDefault(); save(); }} use:inlineKeys>
                  <span class="ov-inl-f"><textarea class="input" id="ov-in-description" aria-label="Description" rows="3" value={form.description ?? ''} oninput={txt('description')} maxlength="4000" {...inv('description', 'ov-in-description')}></textarea><button type="button" class="ux3-info" title="Entrée pour enregistrer, Maj+Entrée pour aller à la ligne." aria-label="Entrée pour enregistrer, Maj+Entrée pour aller à la ligne."><Icon name="info" size="sm" /></button><FieldError message={errors.description} for="ov-in-description" /></span>
                  {@render inlineActs('description')}
                </form>
              {:else}<span class="v ov-pre" class:empty={!p.description}>{p.description ?? 'Aucune description'}</span>{@render pen('description', p.description ? 'Modifier la description' : 'Ajouter une description', !p.description)}{/if}
            </dd>
          </div>
        </dl>
        <p class="muted small ov-upd">Modifié le {dateTime(p.updated_at)}</p>
      </section>
    </div>

    <div class="ov-side">
      <!-- Inventaire -->
      <section class="card ov-inv">
        <div class="card-head"><h2>Inventaire</h2><a class="btn sm" href="/projets/{pid}/inventaire">Ouvrir</a></div>
        {#if elements.length}
          <ul class="ov-sils">
            {#each elements.slice(0, SIL_MAX) as c (c.id)}
              {@const r = c.rackable && c.height_u ? rackSil(c) : null}
              <li>
                <a class="ov-sil" href="/projets/{pid}/inventaire?emplacement={c.location_id}&element={c.id}" title={r ? `${c.name} : ${fu(r.placed)} U posés sur ${r.H} U` : `${c.name} : ${CONTAINER_KINDS[c.kind].label}`}>
                  <span class="vis" aria-hidden="true">
                    {#if r}<span class="mini-rack" class:case={c.kind !== 'rack'} style="height:{r.h}px">{#each Array(r.n) as _, k}<i class:on={k < r.lit} style="--c:var(--ok)"></i>{/each}</span>
                    {:else}<KindIll kind={c.kind} size={52} />{/if}
                  </span>
                  <span class="nm">{c.name}</span>
                  <small class="mono">{r ? `${fu(r.placed)} / ${r.H} U` : CONTAINER_KINDS[c.kind].label.toLowerCase()}</small>
                </a>
              </li>
            {/each}
            {#if elements.length > SIL_MAX}<li class="ov-sil-more"><a class="ov-sil" href="/projets/{pid}/inventaire"><span class="vis plus">+{elements.length - SIL_MAX}</span><span class="nm">autres éléments</span></a></li>{/if}
          </ul>
        {:else}<p class="muted small ov-noel" title="Ajoutez un rack, une valise ou du mobilier depuis l’inventaire.">Aucun élément</p>{/if}
        <div class="ov-kpis">
          <div><b>{nf(pieces)}</b><span>pièce{pieces > 1 ? 's' : ''}</span></div>
          <div><b>{nf(weight, 1)} kg</b><span>poids</span></div>
          <div><b>{nf(power)} W</b><span>consommation</span></div>
        </div>
      </section>

      <!-- Tâches -->
      <section class="card ov-tasks">
        <div class="card-head"><h2>Tâches</h2><a class="btn ghost sm" href="/projets/{pid}/taches">{view.tasks.length ? `${doneTasks} sur ${view.tasks.length} faite${doneTasks > 1 ? 's' : ''}` : 'Ouvrir'}</a></div>
        {#if nextTasks.length}
          <ul class="ov-tk">
            {#each nextTasks as t (t.id)}
              {@const late = !!t.due && t.due < today}
              <li>
                <input type="checkbox" checked={false} disabled={!writable || ticking === t.id} aria-label="Marquer « {t.title} » comme terminée" title="Marquer comme terminée" onchange={() => finish(t)}>
                <span class="tx">{t.title}{#if t.status === 'in_progress'}<span class="muted small"> · en cours</span>{/if}</span>
                {#if t.due}<span class="due mono" class:late title="Échéance : {fdate(t.due, { day: 'numeric', month: 'long', year: 'numeric' })}">{dueText(t.due)}</span>{/if}
              </li>
            {/each}
          </ul>
        {:else}<EmptyState card={false} icon="check" message={view.tasks.length ? 'Toutes les tâches sont faites' : 'Aucune tâche'} hint={view.tasks.length ? '' : 'Les tâches du projet s’ajoutent depuis la page Tâches.'} />{/if}
      </section>

      <!-- Équipe -->
      {#if team.length}
        <section class="card ov-team">
          <div class="card-head"><h2>Équipe</h2><span class="muted small">{online.size} en ligne</span></div>
          <div class="ov-team-b">
            <span class="ov-avs">
              {#each team.slice(0, TEAM_MAX) as m (m.id)}<span class="ov-av" class:on={m.on} title="{m.name}{m.on ? ' (en ligne)' : ''}"><Avatar name={m.name} color={m.color} /></span>{/each}
              {#if team.length > TEAM_MAX}<span class="av ov-av-more" title={team.slice(TEAM_MAX).map((m) => m.name).join(', ')}>+{team.length - TEAM_MAX}</span>{/if}
            </span>
            <span class="muted small">{others.length ? `${others.map((m) => m.name).join(', ')} ${others.length > 1 ? 'consultent' : 'consulte'} aussi le projet.` : 'Personne d’autre en ligne. Les membres ayant modifié le projet récemment apparaissent ici.'}</span>
          </div>
        </section>
      {/if}

      <!-- Activité récente -->
      <section class="card ov-act">
        <div class="card-head"><h2>Activité récente</h2><a class="btn ghost sm" href="/projets/{pid}/historique">Historique</a></div>
        {#if entries === null}<Loading label="Lecture du journal" />
        {:else if !recent.length}<p class="muted small ov-noel">Aucune modification enregistrée.</p>
        {:else}
          <ul class="ov-al">
            {#each recent as e (e.id)}
              <li>
                <Avatar name={nameOf(e.actor_id)} color={colorOf(e.actor_id)} size="sm" />
                <span class="tx"><span class="who"><b>{nameOf(e.actor_id)}</b>{" · "}{e.action_label} · {e.object_label.toLowerCase()}</span>{#if e.subject}<span class="sub" title={e.subject}>{e.subject}</span>{/if}</span>
                <time class="mono" datetime={e.at} title={dateTime(e.at)}>{ago(e.at)}</time>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      <ProjectLinks pid={p.id} chips />
    </div>
  </div>
</div>

{#if dialog && form}
  <Dialog title="Informations du projet" eyebrow={PROJECT_TYPES[projectType(p.regie)].label} onclose={() => { dialog = false; form = null; errors = {}; }} onsubmit={save} submitLabel="Enregistrer" dirty={() => dirty} busy={busy}>
    <div class="form-grid ov-form">
      <div class="field span"><label for="o-name">Nom</label><input class="input" id="o-name" bind:value={form.name} required maxlength="120" {...inv('name', 'o-name')}><FieldError message={errors.name} for="o-name" /></div>
      <div class="field"><label for="o-client">Client (facultatif)</label><input class="input" id="o-client" value={form.client ?? ''} oninput={txt('client')} placeholder="Vide : projet interne" {...inv('client', 'o-client')}><FieldError message={errors.client} for="o-client" /></div>
      <div class="field"><label for="o-venue">Lieu</label><input class="input" id="o-venue" value={form.venue ?? ''} oninput={txt('venue')} {...inv('venue', 'o-venue')}><FieldError message={errors.venue} for="o-venue" /></div>
      <div class="field"><label for="o-ps">Préparation : début</label><input class="input" type="date" id="o-ps" value={form.planning_start ?? ''} oninput={txt('planning_start')} {...inv('planning_start', 'o-ps')}><FieldError message={errors.planning_start} for="o-ps" /></div>
      <div class="field"><label for="o-pe">Préparation : fin</label><input class="input" type="date" id="o-pe" value={form.planning_end ?? ''} oninput={txt('planning_end')} {...inv('planning_end', 'o-pe')}><FieldError message={errors.planning_end} for="o-pe" /></div>
      {#if fixe}
        <div class="field"><label for="o-ss">Mise en service</label><input class="input" type="date" id="o-ss" value={form.shoot_start ?? ''} oninput={commission} {...inv('shoot_start', 'o-ss')}><FieldError message={errors.shoot_start} for="o-ss" /></div>
      {:else}
        <div class="field"><label for="o-ss">Exploitation : début</label><input class="input" type="date" id="o-ss" value={form.shoot_start ?? ''} oninput={txt('shoot_start')} {...inv('shoot_start', 'o-ss')}><FieldError message={errors.shoot_start} for="o-ss" /></div>
        <div class="field"><label for="o-se">Exploitation : fin</label><input class="input" type="date" id="o-se" value={form.shoot_end ?? ''} oninput={txt('shoot_end')} {...inv('shoot_end', 'o-se')}><FieldError message={errors.shoot_end} for="o-se" /></div>
      {/if}
      <div class="field span"><label for="o-desc">Description</label><textarea class="input" id="o-desc" rows="4" value={form.description ?? ''} oninput={txt('description')} {...inv('description', 'o-desc')}></textarea><FieldError message={errors.description} for="o-desc" /></div>
    </div>
  </Dialog>
{/if}

<style>
  /* Mise en page suivant la largeur disponible (barre latérale comprise) : deux colonnes au-delà
     d'environ 900 px de contenu, bandeau empilé sous 600 px. */
  .ov { container-type: inline-size; display: flex; flex-direction: column; gap: 16px; min-width: 0; }

  /* ---- Bandeau ---- */
  .ov-hero { position: relative; overflow: hidden; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 18px 28px; padding: 22px 24px 20px; border-radius: var(--r-md); box-shadow: var(--card-sh);
    background: linear-gradient(135deg, var(--surface) 0%, color-mix(in srgb, var(--accent-soft) 45%, var(--surface)) 60%, var(--accent-soft) 100%); }
  .ov-hero::before { content: ""; position: absolute; inset: 0; pointer-events: none;
    background-image: linear-gradient(color-mix(in srgb, var(--accent) 7%, transparent) 1px, transparent 1px), linear-gradient(90deg, color-mix(in srgb, var(--accent) 7%, transparent) 1px, transparent 1px);
    background-size: 24px 24px; mask-image: linear-gradient(90deg, transparent 0%, #000 55%); }
  .ov-hero > * { position: relative; min-width: 0; }
  .ov-tags { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .ov-tags .chip.ok .dot { background: var(--ok); }
  .ov-name { margin: 10px 0 6px; font: 700 var(--fs-3xl)/1.05 var(--display); letter-spacing: -.02em; overflow-wrap: anywhere; }
  .ov-meta { display: flex; flex-wrap: wrap; gap: 6px 18px; align-items: center; color: var(--fg-2); }
  .ov-meta > span { display: inline-flex; gap: 6px; align-items: center; min-width: 0; }
  .ov-meta :global(.ic) { color: var(--fg-3); }
  .ov-add { display: inline-flex; align-items: center; gap: 4px; padding: 0; border: 0; background: none; color: var(--accent); font: 600 var(--fs-md) var(--font); cursor: pointer; }
  .ov-add :global(.ic) { color: inherit; }
  .ov-add:hover { text-decoration: underline; }
  .ov-desc { margin: 8px 0 0; max-width: 70ch; color: var(--fg-2); display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .ov-count { align-self: start; display: flex; flex-direction: column; align-items: flex-end; gap: 2px; text-align: right; }
  .ov-big { font: 700 var(--fs-3xl)/1 var(--display); letter-spacing: -.03em; font-variant-numeric: tabular-nums; color: var(--c); }
  .ov-big small { margin-right: 2px; font-size: var(--fs-2xl); color: var(--fg-3); }
  .ov-word { font-size: var(--fs-2xl); line-height: 1.15; }
  .ov-lbl { font: 600 var(--fs-sm) var(--font); color: var(--fg-2); }
  .ov-acts { display: flex; gap: 6px; justify-content: flex-end; align-items: center; margin-top: 10px; }

  /* Frise : périodes à l'échelle des dates, trait « Aujourd'hui », graduations. */
  .ov-tl { grid-column: 1 / -1; margin-top: 16px; }
  .ov-track { position: relative; height: 44px; border-radius: 10px; background: color-mix(in srgb, var(--surface) 70%, transparent); box-shadow: inset 0 0 0 1px var(--line); }
  .ov-seg { position: absolute; top: 6px; bottom: 6px; min-width: 6px; display: flex; align-items: center; gap: 8px; padding: 0 10px; overflow: hidden; border-radius: 7px; white-space: nowrap; color: var(--fg); font-size: var(--fs-sm); }
  .ov-track.two { height: 72px; }
  .ov-track.two .ov-seg { bottom: auto; height: 26px; }
  .ov-track.two .l1 { top: 40px; }
  .ov-seg-out { position: absolute; top: 6px; bottom: 6px; display: flex; align-items: center; gap: 8px; white-space: nowrap; font-size: var(--fs-sm); }
  .ov-track.two .ov-seg-out { bottom: auto; height: 26px; }
  .ov-seg b, .ov-seg-out b { font-weight: 650; }
  .ov-seg .mono, .ov-seg-out .mono { font-size: var(--fs-xs); color: var(--fg-2); }
  /* Couleur de statut en repère (liseré et aplat doux), jamais en texte. */
  .ov-seg.prep { background: color-mix(in srgb, var(--st-prep) 16%, var(--surface)); box-shadow: inset 3px 0 0 var(--st-prep); }
  .ov-seg.expl { background: color-mix(in srgb, var(--st-cours) 18%, var(--surface)); box-shadow: inset 3px 0 0 var(--st-cours); }
  .ov-now { position: absolute; top: -6px; bottom: -6px; width: 2px; margin-left: -1px; border-radius: 2px; background: var(--fg); }
  .ov-now span { position: absolute; bottom: calc(100% + 2px); left: 50%; transform: translateX(-50%); font: 600 var(--fs-2xs) var(--font); color: var(--fg); white-space: nowrap; }
  .ov-now.l span { left: 0; transform: none; }
  .ov-now.r span { left: auto; right: 0; transform: none; }
  .ov-scale { position: relative; height: 18px; margin-top: 6px; font: 600 var(--fs-2xs) var(--mono); color: var(--fg-3); font-variant-numeric: tabular-nums; }
  .ov-scale span { position: absolute; top: 0; transform: translateX(-50%); white-space: nowrap; }
  .ov-scale .t0 { transform: none; left: 0 !important; }
  .ov-scale .t4 { transform: none; left: auto !important; right: 0; }

  /* ---- Préparation ---- */
  .ov-ready { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 22px; align-items: center; padding: 18px 20px; }
  .ov-ready > :global(.loading) { grid-column: 1 / -1; }
  .ov-ring { position: relative; width: 132px; height: 132px; }
  .ov-ring svg { width: 100%; height: 100%; transform: rotate(-90deg); }
  .ov-ring circle { fill: none; stroke-width: 12; }
  .ov-ring .trk { stroke: var(--surface-3); }
  .ov-ring .val { stroke: var(--accent); stroke-linecap: round; }
  .ov-ring .val.ok { stroke: var(--ok); }
  @media (prefers-reduced-motion: no-preference) { .ov-ring .val { transition: stroke-dashoffset .8s ease; } }
  .ov-ring-v { position: absolute; inset: 0; display: grid; place-content: center; text-align: center; }
  .ov-ring-v b { font: 700 var(--fs-2xl)/1 var(--display); font-variant-numeric: tabular-nums; }
  .ov-ring-v span { margin-top: 4px; font: 600 var(--fs-2xs) var(--font); letter-spacing: .05em; text-transform: uppercase; color: var(--fg-3); }
  .ov-doms { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 10px; }
  .ov-dom { display: flex; flex-direction: column; gap: 7px; min-width: 0; padding: 12px; border: 0; border-radius: 10px; background: var(--surface-2); color: inherit; text-align: left; font: inherit; text-decoration: none; cursor: pointer; box-shadow: inset 0 0 0 1px transparent; transition: box-shadow .12s, background .12s; }
  .ov-dom:hover { background: var(--surface); box-shadow: inset 0 0 0 1px var(--line-2); }
  .ov-dom .t { display: flex; justify-content: space-between; align-items: center; gap: 6px; font: 600 var(--fs-sm) var(--font); color: var(--fg-2); }
  .ov-dom .t :global(.ic) { color: var(--fg-3); flex: none; }
  .ov-dom b { font: 700 var(--fs-xl)/1 var(--display); font-variant-numeric: tabular-nums; }
  .ov-dom .bar { display: block; height: 5px; border-radius: 3px; background: var(--surface-3); overflow: hidden; }
  .ov-dom .bar i { display: block; height: 100%; border-radius: 3px; background: var(--accent); }
  .ov-dom .bar i.ok { background: var(--ok); }
  .ov-dom .n { font-size: var(--fs-xs); line-height: 1.35; color: var(--fg-3); display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }

  /* ---- Colonnes ---- */
  .ov-cols { display: grid; grid-template-columns: minmax(0, 1.55fr) minmax(0, 1fr); gap: 16px; align-items: start; }
  .ov-side { display: flex; flex-direction: column; gap: 16px; min-width: 0; }
  .ov-side > :global(.card) { margin: 0; }

  /* À faire : liseré de gravité, titre, sous-texte, nombre, chevron. */
  .ov-todo-l { list-style: none; margin: 6px -10px 0; padding: 0; }
  .ov-todo-it { display: grid; grid-template-columns: 4px minmax(0, 1fr) auto auto; gap: 12px; align-items: center; width: 100%; padding: 10px; border: 0; border-radius: var(--r-sm); background: none; color: inherit; font: inherit; text-align: left; text-decoration: none; cursor: pointer; }
  .ov-todo-it:hover { background: color-mix(in srgb, var(--accent) 7%, transparent); }
  .ov-todo-it :global(.ic) { color: var(--fg-3); }
  .sev { align-self: stretch; min-height: 34px; border-radius: 3px; background: var(--line-2); }
  .sev.err { background: var(--err); }
  .sev.warn { background: var(--warn); }
  .tx { display: flex; flex-direction: column; min-width: 0; }
  .tt { font-weight: 600; }
  .ss { display: flex; min-width: 0; font-size: var(--fs-sm); color: var(--fg-3); white-space: nowrap; }
  .ss-h { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .ss-w { flex: none; }
  .num { font-weight: 700; font-variant-numeric: tabular-nums; }
  .ov-more { padding: 4px 0 0; }
  .ov-done { display: flex; flex-direction: column; gap: 6px; margin: 10px -18px -18px; padding: 12px 18px 14px; border-top: 1px solid var(--line); font-size: var(--fs-sm); color: var(--fg-3); }
  .ov-done span { display: inline-flex; gap: 8px; align-items: center; }
  .ov-done :global(.ic) { color: var(--ok); flex: none; }

  /* Informations : fiche en lecture, crayon par ligne. */
  .ov-dl { display: grid; grid-template-columns: 120px minmax(0, 1fr); margin: 4px 0 0; }
  .ov-row { display: contents; }
  .ov-row dt, .ov-row dd { margin: 0; padding: 8px 0; border-top: 1px solid var(--line); min-height: 46px; }
  .ov-row:first-child dt, .ov-row:first-child dd { border-top: 0; }
  .ov-row dt { display: flex; align-items: center; color: var(--fg-3); font-size: var(--fs-sm); }
  .ov-row dd { display: flex; justify-content: space-between; gap: 8px; align-items: center; min-width: 0; }
  .ov-row .v { min-width: 0; overflow-wrap: anywhere; }
  .ov-row .v.empty { color: var(--fg-3); font-style: italic; }
  .ov-pre { white-space: pre-line; }
  .ov-pen { flex: none; color: var(--fg-3); }
  @media (hover: hover) {
    .ov-pen { opacity: 0; transition: opacity .12s; }
    .ov-row:hover .ov-pen, .ov-pen:focus-visible { opacity: 1; }
  }
  .ov-inl { display: flex; gap: 8px; align-items: flex-start; width: 100%; min-width: 0; }
  .ov-inl-f { flex: 1; display: grid; gap: 4px; min-width: 0; }
  .ov-dates { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .ov-dates :global(.help) { grid-column: 1 / -1; }
  .ov-inl-acts { display: flex; gap: 4px; flex: none; }
  .ov-ok { color: var(--accent); }
  .ov-upd { margin: 10px 0 0; }

  /* Inventaire : silhouettes des éléments (vignette de l'inventaire), mobilier en pictogramme. */
  .ov-sils { display: flex; flex-wrap: wrap; gap: 6px 4px; align-items: flex-end; margin: 6px 0 0; padding: 0; list-style: none; }
  .ov-sil { display: flex; flex-direction: column; align-items: center; gap: 5px; width: 76px; padding: 6px 2px; border-radius: var(--r-sm); color: inherit; text-decoration: none; }
  .ov-sil:hover { background: color-mix(in srgb, var(--accent) 7%, transparent); }
  .ov-sil .vis { display: grid; place-items: end center; min-height: 40px; color: var(--fg-2); }
  .ov-sil .vis.plus { place-items: center; width: 44px; height: 40px; border-radius: var(--r-sm); background: var(--surface-3); font: 700 var(--fs-sm) var(--mono); color: var(--fg-2); }
  .ov-sil .mini-rack { width: 30px; padding: 4px 5px 5px; gap: 1px; box-shadow: inset 0 0 0 1px rgba(0, 0, 0, .6), 0 6px 10px -6px rgba(0, 0, 0, .45); }
  .ov-sil .mini-rack.case { padding: 5px 6px 6px; box-shadow: inset 0 0 0 2px var(--ill-alu), 0 6px 10px -6px rgba(0, 0, 0, .45); }
  .ov-sil .mini-rack.case::after { display: none; }
  .ov-sil .mini-rack i { flex: 1 1 0; min-height: 1px; opacity: .38; }
  .ov-sil .mini-rack i::before { display: none; }
  .ov-sil .mini-rack i.on { opacity: 1; background: linear-gradient(180deg, color-mix(in srgb, var(--ok) 70%, #fff), var(--ok)); }
  .ov-sil .nm { max-width: 100%; font: 600 var(--fs-2xs)/1.25 var(--font); color: var(--fg-2); text-align: center; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ov-sil small { font-size: var(--fs-2xs); color: var(--fg-3); white-space: nowrap; }
  .ov-noel { margin: 10px 0 0; }
  .ov-kpis { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin-top: 12px; }
  .ov-kpis div { min-width: 0; padding: 10px; border-radius: 9px; background: var(--surface-2); }
  .ov-kpis b { display: block; font: 700 var(--fs-lg) var(--display); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .ov-kpis span { font-size: var(--fs-xs); color: var(--fg-3); }

  /* Tâches */
  .ov-tk, .ov-al { list-style: none; margin: 4px -8px 0; padding: 0; }
  .ov-tk li { display: grid; grid-template-columns: 16px minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 8px; border-radius: var(--r-sm); }
  .ov-tk li:hover { background: var(--surface-2); }
  .ov-tk input { width: 16px; height: 16px; margin: 0; accent-color: var(--btn-pri); cursor: pointer; }
  .ov-tk input:disabled { cursor: not-allowed; }
  .ov-tk .tx { display: block; overflow-wrap: anywhere; }
  .due { font-size: var(--fs-xs); color: var(--fg-3); white-space: nowrap; }
  .due.late { color: var(--err); font-weight: 700; }

  /* Équipe */
  .ov-team-b { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin-top: 10px; }
  .ov-avs { display: flex; padding-left: 8px; }
  .ov-av, .ov-av-more { position: relative; margin-left: -8px; }
  .ov-av :global(.av), .ov-av-more { box-shadow: 0 0 0 2px var(--surface); }
  .ov-av-more { background: var(--surface-3); color: var(--fg-2); text-shadow: none; }
  .ov-av.on::after { content: ""; position: absolute; right: -1px; bottom: -1px; width: 9px; height: 9px; border-radius: 50%; background: var(--ok); box-shadow: 0 0 0 2px var(--surface); }

  /* Activité */
  .ov-al li { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 7px 8px; font-size: var(--fs-sm); }
  .ov-al .tx { display: flex; flex-direction: column; min-width: 0; color: var(--fg-3); }
  .ov-al .tx b { color: var(--fg); font-weight: 600; }
  .ov-al .sub { color: var(--fg); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ov-al time { font-size: var(--fs-2xs); color: var(--fg-3); white-space: nowrap; }

  .ov-form { margin-top: 16px; }

  /* Largeur du contenu : une colonne, puis bandeau empilé. */
  @container (max-width: 980px) {
    .ov-doms { grid-template-columns: repeat(3, minmax(0, 1fr)); }
    .ov-cols { grid-template-columns: minmax(0, 1fr); }
  }
  @container (max-width: 640px) {
    .ov-hero { grid-template-columns: minmax(0, 1fr); padding: 18px 16px; }
    .ov-name { font-size: var(--fs-2xl); }
    .ov-count { align-items: flex-start; text-align: left; }
    .ov-acts { justify-content: flex-start; }
    .ov-ready { grid-template-columns: minmax(0, 1fr); justify-items: center; padding: 16px; }
    .ov-doms { grid-template-columns: repeat(2, minmax(0, 1fr)); width: 100%; }
    .ov-dom:last-child:nth-child(odd) { grid-column: 1 / -1; }
    .ov-scale .t1, .ov-scale .t3 { display: none; }
    .ov-seg .mono, .ov-seg-out .mono { display: none; }
    .ov-dl { grid-template-columns: 96px minmax(0, 1fr); }
    .ov-dates { grid-template-columns: minmax(0, 1fr); }
  }
</style>
