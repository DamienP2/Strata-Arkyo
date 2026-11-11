<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Ill from '../../components/Ill.svelte';
  import Vu from '../../components/Vu.svelte';
  import StatusChip from '../../components/StatusChip.svelte';
  import Wizard from './Wizard.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { fail } from '../../lib/toast.svelte';
  import { route } from '../../lib/router.svelte';
  import { STATUS, STATUS_ORDER, REGIE, keyDate, daysBetween, fdate, urgency, type ProjectSummary, type Status, type Regie, type Intervention } from '../../lib/projects';

  // Recherche, tri et filtres mémorisés sur cet appareil.
  const saved = (() => { try { return JSON.parse(localStorage.getItem('sa-projects') ?? '{}'); } catch { return {}; } })();
  let f = $state<{ inter: Intervention; q: string; st: Status[]; regie: Regie[]; sort: 'updated' | 'name' }>({ inter: 'prestation', q: '', st: [], regie: [], sort: 'updated', ...saved });
  $effect(() => { const v = JSON.stringify(f); try { localStorage.setItem('sa-projects', v); } catch { /* indisponible */ } });

  let projects = $state<ProjectSummary[]>([]);
  let today = $state(new Date().toISOString().slice(0, 10));
  let loading = $state(true);
  let wizard = $state(route.query.get('nouveau') === '1');
  api.get<{ projects: ProjectSummary[]; today: string }>('/projects').then((r) => { projects = r.projects; today = r.today; }).catch(fail).finally(() => (loading = false));

  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const list = $derived.by(() => {
    const q = norm(f.q.trim());
    let l = projects.filter((p) => p.intervention === f.inter && (!f.st.length || f.st.includes(p.status)) && (!f.regie.length || f.regie.includes(p.regie)) && (!q || norm([p.name, p.client, p.venue, p.service_type, p.description].filter(Boolean).join(' ')).includes(q)));
    l = f.sort === 'name' ? [...l].sort((a, b) => a.name.localeCompare(b.name, 'fr')) : [...l].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
    return l;
  });
  const groups = $derived(STATUS_ORDER.map((s) => ({ s, items: list.filter((p) => p.status === s) })).filter((g) => g.items.length));
  const filters = $derived(f.st.length + f.regie.length);
  const toggle = <T,>(arr: T[], v: T) => (arr.includes(v) ? arr.filter((x) => x !== v) : [...arr, v]);
  const clients = $derived([...new Set(projects.map((p) => p.client).filter(Boolean))] as string[]);
</script>

<div class="page">
  <div class="page-head">
    <div><p class="eyebrow">{projects.filter((p) => !['done', 'cancelled'].includes(p.status)).length} projets actifs sur {projects.length}</p><h1 class="display">Projets</h1></div>
    {#if can('projets', 'ecriture')}<div class="head-actions"><button class="btn primary" onclick={() => (wizard = true)}><Icon name="plus" />Nouveau projet</button></div>{/if}
  </div>

  <div class="toolbar" style="display:flex;gap:10px;flex-wrap:wrap;align-items:center">
    <div class="seg">{#each [['prestation', 'Prestations'], ['integration', 'Intégrations']] as [k, l]}<button class:on={f.inter === k} onclick={() => (f.inter = k as Intervention)}>{l} <span class="muted">{projects.filter((p) => p.intervention === k).length}</span></button>{/each}</div>
    <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={f.q} placeholder="Nom, client, lieu, type de prestation…" aria-label="Rechercher un projet"></label>
    <div class="seg">{#each [['updated', 'Récents'], ['name', 'A → Z']] as [k, l]}<button class:on={f.sort === k} onclick={() => (f.sort = k as typeof f.sort)}>{l}</button>{/each}</div>
  </div>
  <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:-24px">
    {#each STATUS_ORDER as s}<button class="fchip" class:on={f.st.includes(s)} style="--c:{STATUS[s].color}" onclick={() => (f.st = toggle(f.st, s))}>{STATUS[s].label}</button>{/each}
    {#each Object.entries(REGIE) as [k, r]}{#if f.inter === 'integration' || k !== 'fixed'}<button class="fchip cxf" class:on={f.regie.includes(k as Regie)} onclick={() => (f.regie = toggle(f.regie, k as Regie))}>{r.label}</button>{/if}{/each}
    {#if filters}<button class="btn ghost sm" onclick={() => { f.st = []; f.regie = []; }}>Effacer {filters} filtre{filters > 1 ? 's' : ''}</button>{/if}
  </div>

  {#if loading}<span class="spin"></span>
  {:else if !groups.length}
    <div class="empty">{projects.length ? 'Aucun projet ne correspond à ces filtres.' : 'Aucun projet pour l’instant.'}{#if !projects.length && can('projets', 'ecriture')} <button class="ilink" onclick={() => (wizard = true)}>Créer le premier</button>{/if}</div>
  {:else}
    <div class="pgroups" style="display:flex;flex-wrap:wrap;gap:40px 24px">
      {#each groups as g (g.s)}
        <section class="pgroup" style="--n:{Math.min(g.items.length, 4)}">
          <div class="group-h"><StatusChip status={g.s} /><span class="count">{g.items.length}</span></div>
          <div class="proj-grid">
            {#each g.items as p (p.id)}
              {@const kd = keyDate(p)}
              {@const d = kd ? daysBetween(today, kd) : null}
              {@const act = !['done', 'cancelled'].includes(p.status)}
              <a class="pcard" class:off={!act} href="/projets/{p.id}" style="--c:{STATUS[p.status].color};text-decoration:none">
                <span class="pc-vis"><Ill kind={REGIE[p.regie].ill} /><span class="pc-tag lbl-t">{REGIE[p.regie].label}</span>
                  <span class="pc-j">{#if act && d != null && d >= 0}<span class="tally" style="--c:{urgency(d)}">J-{d}</span>{:else if !act}<span class="tally off">{p.status === 'done' ? 'Livré' : 'Annulé'}</span>{/if}</span></span>
                <span class="pc-body"><span class="lbl-t">{p.service_type ?? '—'} · {p.intervention === 'prestation' ? 'Prestation' : 'Intégration'}</span><b class="pc-name">{p.name}</b><span class="pc-sub">{p.client ?? 'Projet interne'}{p.venue ? ` · ${p.venue}` : ''}</span>
                  <span class="pc-foot"><span><span class="lbl-t">{p.intervention === 'prestation' ? 'Tournage' : 'Mise en service'}</span><b>{fdate(kd, { day: 'numeric', month: 'short', year: 'numeric' })}</b></span><span><span class="lbl-t">Fiche · {p.completion} %</span><Vu value={p.completion} n={20} cls={p.completion >= 80 ? 'ok' : ''} /></span></span></span>
              </a>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {/if}
</div>

{#if wizard}<Wizard {clients} onclose={() => (wizard = false)} />{/if}
