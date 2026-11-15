<script lang="ts">
  // Recherche et actions rapides au clavier (section 12, Ctrl+K) : écrans (« Aller à »),
  // projets, équipements et câbles du projet ouvert, fiches du catalogue, membres et actions ;
  // le tout filtré par les droits de la personne. Un résultat ouvre l'objet lui-même (paramètre
  // de ciblage dans l'adresse), pas seulement la liste. La palette entre dans la pile des
  // boîtes (modal.ts) : Échap ne ferme qu'elle, le focus revient au déclencheur.
  import Icon from './Icon.svelte';
  import { api } from '../lib/api';
  import { modal } from '../lib/modal';
  import { can } from '../lib/session.svelte';
  import { route, navigate } from '../lib/router.svelte';

  let { open = $bindable(false) }: { open?: boolean } = $props();
  interface Hit { group: string; label: string; sub: string; href: string; icon: string }
  let q = $state('');
  let hits = $state<Hit[]>([]);
  let cur = $state(0);
  let timer: ReturnType<typeof setTimeout>;
  // Données lentes à charger, gardées le temps de la palette.
  let projects: { id: string; name: string; client: string | null; venue: string | null }[] | null = null;
  let members: { id: string; name: string; email: string }[] | null = null;
  let proj: { id: string; articles: any[]; instances: any[]; containers: any[]; locations: any[] } | null = null;
  let cables: { id: string; number: string | null; signal: string; a: any; b: any }[] | null = null;

  const pid = $derived(route.path.startsWith('/projets/') ? route.path.split('/')[2] ?? null : null);
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const match = (t: string, terms: string[]) => { const n = norm(t); return terms.every((x) => n.includes(x)); };

  // Écrans du projet ouvert, dans l'ordre de la navigation du projet, puis sections de l'application.
  const SCREENS: [string, string, string][] = [
    ['', 'Vue d’ensemble', 'grid'], ['inventaire', 'Inventaire', 'box'], ['taches', 'Tâches', 'check'],
    ['racks', 'Racks', 'server'], ['mobilier', 'Mobilier', 'layers'], ['planche', 'Planches', 'sheet'], ['synoptique', 'Synoptique', 'syn'],
    ['cables', 'Liste des câbles', 'cable'], ['reseau', 'Réseau', 'network'], ['plans', 'Plans', 'map'], ['dessins', 'Dessins', 'pen'],
    ['preparation', 'Préparation', 'truck'], ['atelier', 'Atelier', 'tools'], ['recette', 'Recette', 'ok'],
    ['dossier', 'Dossier et exports', 'file'], ['historique', 'Historique', 'clock'],
  ];
  function screens(): Hit[] {
    const a: Hit[] = [];
    if (pid && can('projets', 'lecture')) for (const [k, l, ic] of SCREENS) a.push({ group: 'Aller à', label: l, sub: 'Projet ouvert', href: `/projets/${pid}${k ? `/${k}` : ''}`, icon: ic });
    a.push({ group: 'Aller à', label: 'Accueil', sub: 'Tableau de bord', href: '/', icon: 'home' });
    if (can('projets', 'lecture')) a.push({ group: 'Aller à', label: 'Projets', sub: 'Liste des projets', href: '/projets', icon: 'folder' });
    if (can('catalogue', 'lecture')) a.push({ group: 'Aller à', label: 'Catalogue', sub: 'Équipements, câbles, référentiels', href: '/catalogue', icon: 'box' });
    if (can('membres', 'admin') || can('parametres', 'admin')) a.push({ group: 'Aller à', label: 'Administration', sub: 'Membres, rôles, sauvegardes', href: '/admin', icon: 'shield' });
    return a;
  }

  function actions(): Hit[] {
    const a: Hit[] = [];
    if (can('projets', 'ecriture')) a.push({ group: 'Actions', label: 'Nouveau projet', sub: 'Assistant de création', href: '/projets?nouveau=1', icon: 'plus' });
    if (pid && can('projets', 'ecriture')) a.push({ group: 'Actions', label: 'Ajouter du matériel', sub: 'Inventaire du projet ouvert', href: `/projets/${pid}/inventaire`, icon: 'box' });
    if (pid) a.push({ group: 'Actions', label: 'Valider le câblage', sub: 'Anomalies du synoptique', href: `/projets/${pid}/synoptique`, icon: 'check' });
    if (pid) a.push({ group: 'Actions', label: 'Exporter le dossier', sub: 'Assistant de dossier', href: `/projets/${pid}/dossier`, icon: 'download' });
    if (pid) a.push({ group: 'Actions', label: 'Recette', sub: 'Procès-verbal et verdicts', href: `/projets/${pid}/recette`, icon: 'ok' });
    if (can('catalogue', 'ecriture')) a.push({ group: 'Actions', label: 'Nouvelle fiche équipement', sub: 'Catalogue', href: '/catalogue/equipment/nouveau', icon: 'plus' });
    if (can('catalogue', 'lecture')) a.push({ group: 'Actions', label: 'Modifier une fiche', sub: 'Catalogue des équipements', href: '/catalogue/equipment', icon: 'pen' });
    return a;
  }

  async function search() {
    const terms = norm(q.trim()).split(/\s+/).filter(Boolean);
    const out: Hit[] = terms.length ? screens().filter((h) => match(h.label, terms)).slice(0, 6) : [];
    out.push(...actions().filter((h) => !terms.length || match(`${h.label} ${h.sub}`, terms)));
    try {
      if (can('projets', 'lecture')) {
        projects ??= (await api.get<{ projects: any[] }>('/projects')).projects;
        out.push(...projects.filter((p) => !terms.length || match(`${p.name} ${p.client ?? ''} ${p.venue ?? ''}`, terms)).slice(0, terms.length ? 8 : 4).map((p) => ({ group: 'Projets', label: p.name, sub: [p.client, p.venue].filter(Boolean).join(' · '), href: `/projets/${p.id}`, icon: 'folder' })));
      }
      if (pid && terms.length && can('projets', 'lecture')) {
        if (proj?.id !== pid) { const v = await api.get<any>(`/projects/${pid}`); proj = { id: pid, articles: v.articles, instances: v.instances, containers: v.containers, locations: v.locations ?? [] }; cables = null; }
        const cname = (id: string | null) => proj!.containers.find((c: any) => c.id === id)?.name ?? 'Matériel libre';
        const lname = (id: string | null) => proj!.locations.find((l: any) => l.id === id)?.name ?? '';
        // Ciblage : l'inventaire ouvre l'emplacement et l'élément, puis met l'exemplaire en évidence.
        const target = (a: any, i: any) => `/projets/${pid}/inventaire?${new URLSearchParams({ emplacement: a.location_id, ...(a.container_id ? { element: a.container_id } : {}), i: i.id })}`;
        for (const i of proj!.instances) {
          const a = proj!.articles.find((x: any) => x.id === i.article_id); if (!a) continue;
          if (match(`${a.label} ${i.tag} ${i.label ?? ''} ${i.serial ?? ''} ${cname(a.container_id)}`, terms)) out.push({ group: 'Équipements du projet', label: `${i.label || a.label} ${i.tag}`, sub: [cname(a.container_id), lname(a.location_id), i.serial ? `N° ${i.serial}` : ''].filter(Boolean).join(' · '), href: target(a, i), icon: 'server' });
          if (out.filter((h) => h.group === 'Équipements du projet').length >= 8) break;
        }
        cables ??= (await api.get<{ rows: any[] }>(`/projects/${pid}/cables/table`)).rows;
        const ends = (c: { a: any; b: any }) => `${c.a.equipment} ${c.a.tag} → ${c.b.equipment} ${c.b.tag}`;
        out.push(...cables.filter((c) => match(`${c.number ?? ''} ${c.signal} ${c.a.equipment} ${c.a.tag} ${c.b.equipment} ${c.b.tag}`, terms)).slice(0, 8).map((c) => ({ group: 'Câbles du projet', label: c.number ?? ends(c), sub: c.number ? `${ends(c)} · ${c.signal}` : `Sans numéro · ${c.signal}`, href: `/projets/${pid}/cables?cable=${encodeURIComponent(c.id)}`, icon: 'cable' })));
      }
      if (terms.length && can('catalogue', 'lecture')) {
        const r = await api.get<{ items: any[] }>(`/catalog/items?${new URLSearchParams({ q: q.trim(), limit: '8', offset: '0' })}`);
        const path: Record<string, string> = { equipment: 'equipment', cable: 'cables', cable_model: 'cable-models', adapter: 'adapters' };
        const kind: Record<string, string> = { equipment: 'Équipement', cable: 'Câble', cable_model: 'Type de câble', adapter: 'Adaptateur' };
        out.push(...r.items.map((i) => ({ group: 'Catalogue', label: i.name, sub: [kind[i.kind] ?? 'Fiche', i.reference].filter(Boolean).join(' · '), href: `/catalogue/${path[i.kind] ?? 'equipment'}/${i.id}`, icon: 'box' })));
      }
      if (terms.length && can('membres', 'admin')) {
        members ??= ((await api.get<any>('/members')).members ?? []);
        out.push(...members!.filter((m) => match(`${m.name} ${m.email}`, terms)).slice(0, 5).map((m) => ({ group: 'Membres', label: m.name, sub: m.email, href: `/admin/membres?membre=${encodeURIComponent(m.id)}`, icon: 'users' })));
      }
    } catch { /* recherche partielle : on montre ce qui a répondu */ }
    hits = out; cur = 0;
  }
  $effect(() => { if (open) { q; clearTimeout(timer); timer = setTimeout(search, 120); } });
  $effect(() => { if (!open) { q = ''; projects = null; members = null; } });
  function go(h: Hit | undefined) { if (!h) return; open = false; navigate(h.href); }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') { e.preventDefault(); cur = Math.min(hits.length - 1, cur + 1); }
    if (e.key === 'ArrowUp') { e.preventDefault(); cur = Math.max(0, cur - 1); }
    if (e.key === 'Enter') { e.preventDefault(); go(hits[cur]); }
  }
  const groups = $derived([...new Set(hits.map((h) => h.group))]);
  $effect(() => { if (open) document.getElementById(`cp-o${cur}`)?.scrollIntoView({ block: 'nearest' }); });
</script>

{#if open}
  <div class="modal" role="presentation" use:modal={{ close: () => (open = false) }}>
    <div class="cp" role="dialog" aria-modal="true" aria-label="Recherche et actions" tabindex="-1">
      <label class="cp-in"><Icon name="search" /><input type="search" data-autofocus bind:value={q} placeholder="Écrans, projets, équipements, câbles, catalogue, membres" onkeydown={key} aria-label="Rechercher" role="combobox" aria-expanded="true" aria-controls="cp-list" aria-activedescendant={hits[cur] ? `cp-o${cur}` : undefined} autocomplete="off"><kbd>Échap</kbd></label>
      <div class="cp-list" id="cp-list" role="listbox" aria-label="Résultats">
        {#each groups as g}
          <div class="cp-g eyebrow" role="presentation">{g}</div>
          {#each hits.map((h, i) => [h, i] as const).filter(([h]) => h.group === g) as [h, i]}
            <button type="button" id="cp-o{i}" role="option" tabindex="-1" aria-selected={i === cur} class:on={i === cur} onmouseenter={() => (cur = i)} onclick={() => go(h)}><Icon name={h.icon} size="sm" /><span class="l"><b>{h.label}</b>{#if h.sub}<span class="muted small">{h.sub}</span>{/if}</span></button>
          {/each}
        {:else}<p class="muted small cp-none">{q.trim() ? 'Aucun résultat.' : 'Tapez le nom d’un écran, d’un projet, d’un équipement ou d’un câble.'}</p>{/each}
      </div>
      <div class="cp-foot muted small"><span><kbd>↑</kbd><kbd>↓</kbd> choisir</span><span><kbd>Entrée</kbd> ouvrir</span><span><kbd>Ctrl</kbd>+<kbd>K</kbd> partout</span></div>
    </div>
  </div>
{/if}

<style>
  .cp { width: min(640px, calc(100vw - 32px)); margin: 10vh auto 0; background: var(--surface); border-radius: var(--r-xl, 24px); box-shadow: var(--e3); overflow: hidden; display: flex; flex-direction: column; max-height: 72vh; }
  .cp-in { display: flex; gap: 10px; align-items: center; padding: 14px 18px; border-bottom: 1px solid var(--line); color: var(--fg-3); }
  /* Le champ remplit la ligne : le focus est signalé par un trait d'accent sous la ligne entière. */
  .cp-in:focus-within { box-shadow: inset 0 -2px 0 var(--accent); }
  .cp-in input { flex: 1; border: 0; background: none; font: 400 var(--fs-lg, 16px) var(--font); color: var(--fg); outline: none; min-width: 0; -webkit-appearance: none; appearance: none; }
  .cp-in input::-webkit-search-cancel-button { display: none; }
  .cp-list { overflow: auto; padding: 6px; }
  .cp-g { margin: 0; padding: 10px 10px 4px; }
  .cp-list button { display: flex; gap: 10px; align-items: center; width: 100%; text-align: left; border: 0; background: none; padding: 8px 10px; border-radius: var(--r-sm, 8px); font: inherit; color: inherit; cursor: pointer; }
  .cp-list button.on { background: color-mix(in srgb, var(--accent) 10%, transparent); }
  .cp-list button :global(.ic) { flex: none; color: var(--fg-3); }
  .cp-none { padding: 14px; margin: 0; }
  .l { display: flex; flex-direction: column; min-width: 0; }
  .l b { font-weight: 600; }
  .cp-foot { display: flex; gap: 16px; padding: 8px 14px; border-top: 1px solid var(--line); flex-wrap: wrap; }
  .cp kbd { margin: 0 2px; }
</style>
