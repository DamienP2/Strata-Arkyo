<script lang="ts">
  // Recherche et actions rapides au clavier (section 12, Ctrl+K) : projets, équipements et
  // câbles du projet ouvert, fiches du catalogue, utilisateurs, et actions ; le tout filtré
  // par les droits de la personne.
  import Icon from './Icon.svelte';
  import { api } from '../lib/api';
  import { can } from '../lib/session.svelte';
  import { route, navigate } from '../lib/router.svelte';

  let { open = $bindable(false) }: { open?: boolean } = $props();
  interface Hit { group: string; label: string; sub: string; href: string; icon: string }
  let q = $state('');
  let hits = $state<Hit[]>([]);
  let cur = $state(0);
  let input = $state<HTMLInputElement>(null!);
  let timer: ReturnType<typeof setTimeout>;
  // Données lentes à charger, gardées le temps de la palette.
  let projects: { id: string; name: string; client: string | null; venue: string | null }[] | null = null;
  let members: { id: string; name: string; email: string }[] | null = null;
  let proj: { id: string; articles: any[]; instances: any[]; containers: any[] } | null = null;
  let cables: { id: string; number: string | null; signal: string; a: any; b: any }[] | null = null;

  const pid = $derived(route.path.startsWith('/projets/') ? route.path.split('/')[2] ?? null : null);
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const match = (t: string, terms: string[]) => { const n = norm(t); return terms.every((x) => n.includes(x)); };

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
    const out: Hit[] = actions().filter((h) => !terms.length || match(`${h.label} ${h.sub}`, terms));
    try {
      if (can('projets', 'lecture')) {
        projects ??= (await api.get<{ projects: any[] }>('/projects')).projects;
        out.push(...projects.filter((p) => !terms.length || match(`${p.name} ${p.client ?? ''} ${p.venue ?? ''}`, terms)).slice(0, terms.length ? 8 : 4).map((p) => ({ group: 'Projets', label: p.name, sub: [p.client, p.venue].filter(Boolean).join(' · '), href: `/projets/${p.id}`, icon: 'folder' })));
      }
      if (pid && terms.length && can('projets', 'lecture')) {
        if (proj?.id !== pid) { const v = await api.get<any>(`/projects/${pid}`); proj = { id: pid, articles: v.articles, instances: v.instances, containers: v.containers }; cables = null; }
        const cname = (id: string | null) => proj!.containers.find((c: any) => c.id === id)?.name ?? 'Équipement libre';
        for (const i of proj!.instances) {
          const a = proj!.articles.find((x: any) => x.id === i.article_id); if (!a) continue;
          if (match(`${a.label} ${i.tag} ${i.serial ?? ''} ${cname(a.container_id)}`, terms)) out.push({ group: 'Équipements du projet', label: `${a.label} ${i.tag}`, sub: cname(a.container_id) + (i.serial ? ` · N° ${i.serial}` : ''), href: `/projets/${pid}/inventaire`, icon: 'server' });
          if (out.filter((h) => h.group === 'Équipements du projet').length >= 8) break;
        }
        cables ??= (await api.get<{ rows: any[] }>(`/projects/${pid}/cables/table`)).rows;
        out.push(...cables.filter((c) => match(`${c.number ?? ''} ${c.signal} ${c.a.equipment} ${c.a.tag} ${c.b.equipment} ${c.b.tag}`, terms)).slice(0, 8).map((c) => ({ group: 'Câbles du projet', label: c.number ?? 'Câble sans numéro', sub: `${c.a.equipment} ${c.a.tag} » ${c.b.equipment} ${c.b.tag} · ${c.signal}`, href: `/projets/${pid}/cables`, icon: 'syn' })));
      }
      if (terms.length && can('catalogue', 'lecture')) {
        const r = await api.get<{ items: any[] }>(`/catalog/items?${new URLSearchParams({ q: q.trim(), limit: '8', offset: '0' })}`);
        const path: Record<string, string> = { equipment: 'equipment', cable: 'cables', cable_model: 'cable-models', adapter: 'adapters' };
        out.push(...r.items.map((i) => ({ group: 'Catalogue', label: i.name, sub: [i.reference, i.kind === 'equipment' ? 'Équipement' : i.kind === 'adapter' ? 'Adaptateur' : 'Câble'].filter(Boolean).join(' · '), href: `/catalogue/${path[i.kind] ?? 'equipment'}/${i.id}`, icon: 'box' })));
      }
      if (terms.length && can('membres', 'admin')) {
        members ??= ((await api.get<any>('/members')).members ?? []);
        out.push(...members!.filter((m) => match(`${m.name} ${m.email}`, terms)).slice(0, 5).map((m) => ({ group: 'Utilisateurs', label: m.name, sub: m.email, href: '/admin/membres', icon: 'users' })));
      }
    } catch { /* recherche partielle : on montre ce qui a répondu */ }
    hits = out; cur = 0;
  }
  $effect(() => { if (open) { q; clearTimeout(timer); timer = setTimeout(search, 120); } });
  $effect(() => { if (open) { setTimeout(() => input?.focus(), 0); } else { q = ''; projects = null; members = null; } });
  function go(h: Hit | undefined) { if (!h) return; open = false; navigate(h.href); }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') { e.preventDefault(); cur = Math.min(hits.length - 1, cur + 1); }
    if (e.key === 'ArrowUp') { e.preventDefault(); cur = Math.max(0, cur - 1); }
    if (e.key === 'Enter') { e.preventDefault(); go(hits[cur]); }
    if (e.key === 'Escape') open = false;
  }
  const groups = $derived([...new Set(hits.map((h) => h.group))]);
</script>

{#if open}
  <div class="modal" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) open = false; }}>
    <div class="cp" role="dialog" aria-modal="true" aria-label="Recherche et actions">
      <label class="cp-in"><Icon name="search" /><input bind:this={input} bind:value={q} placeholder="Projets, équipements, câbles, fiches, personnes, actions…" onkeydown={key} aria-label="Rechercher"><kbd>Échap</kbd></label>
      <div class="cp-list" role="listbox" aria-label="Résultats">
        {#each groups as g}
          <div class="cp-g">{g}</div>
          {#each hits.map((h, i) => [h, i] as const).filter(([h]) => h.group === g) as [h, i]}
            <button role="option" aria-selected={i === cur} class:on={i === cur} onmouseenter={() => (cur = i)} onclick={() => go(h)}><Icon name={h.icon} size="sm" /><span class="l"><b>{h.label}</b>{#if h.sub}<span class="muted small">{h.sub}</span>{/if}</span></button>
          {/each}
        {:else}<p class="muted small" style="padding:14px">Aucun résultat.</p>{/each}
      </div>
      <div class="cp-foot muted small"><span><kbd>↑</kbd><kbd>↓</kbd> choisir</span><span><kbd>Entrée</kbd> ouvrir</span><span><kbd>Ctrl</kbd>+<kbd>K</kbd> partout</span></div>
    </div>
  </div>
{/if}

<style>
  .cp { width: min(640px, calc(100vw - 32px)); margin: 10vh auto 0; background: var(--surface); border-radius: 18px; box-shadow: 0 30px 80px -20px rgba(0, 0, 0, .45); overflow: hidden; display: flex; flex-direction: column; max-height: 72vh; }
  .cp-in { display: flex; gap: 10px; align-items: center; padding: 14px 16px; border-bottom: 1px solid var(--line); }
  .cp-in input { flex: 1; border: 0; background: none; font: 500 17px var(--font); color: var(--fg); outline: none; min-width: 0; }
  .cp-list { overflow: auto; padding: 6px; }
  .cp-g { font: 650 11px var(--font); text-transform: uppercase; letter-spacing: .05em; color: var(--fg-2); padding: 10px 10px 4px; }
  .cp-list button { display: flex; gap: 10px; align-items: center; width: 100%; text-align: left; border: 0; background: none; padding: 8px 10px; border-radius: 10px; font: inherit; color: inherit; cursor: pointer; }
  .cp-list button.on { background: var(--surface-2); }
  .l { display: flex; flex-direction: column; min-width: 0; }
  .l b { font-weight: 600; }
  .cp-foot { display: flex; gap: 16px; padding: 8px 14px; border-top: 1px solid var(--line); flex-wrap: wrap; }
  kbd { font: 600 11px var(--font-mono, monospace); padding: 1px 5px; border: 1px solid var(--line); border-radius: 5px; background: var(--surface-2); margin: 0 2px; }
</style>
