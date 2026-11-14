<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Ill from '../../components/Ill.svelte';
  import Vu from '../../components/Vu.svelte';
  import StatusChip from '../../components/StatusChip.svelte';
  import Wizard from './Wizard.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { STATUS, STATUS_ORDER, REGIE, keyDate, daysBetween, fdate, period, urgency, type ProjectSummary, type Status, type Regie } from '../../lib/projects';

  // Recherche, tri et filtres mémorisés sur cet appareil.
  const saved = (() => { try { return JSON.parse(localStorage.getItem('sa-projects') ?? '{}'); } catch { return {}; } })();
  let f = $state<{ q: string; st: Status[]; regie: Regie[]; sort: 'updated' | 'name' | 'date'; view: 'cards' | 'list' }>({ q: '', st: [], regie: [], sort: 'updated', view: 'cards', ...saved });
  $effect(() => { const v = JSON.stringify(f); try { localStorage.setItem('sa-projects', v); } catch { /* indisponible */ } });

  let projects = $state<ProjectSummary[]>([]);
  let today = $state(new Date().toISOString().slice(0, 10));
  let loading = $state(true);
  let wizard = $state(route.query.get('nouveau') === '1');
  api.get<{ projects: ProjectSummary[]; today: string }>('/projects').then((r) => { projects = r.projects; today = r.today; }).catch(fail).finally(() => (loading = false));

  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const list = $derived.by(() => {
    const q = norm(f.q.trim());
    let l = projects.filter((p) => (!f.st.length || f.st.includes(p.status)) && (!f.regie.length || f.regie.includes(p.regie)) && (!q || norm([p.name, p.client, p.venue, p.service_type, p.description].filter(Boolean).join(' ')).includes(q)));
    l = f.sort === 'name' ? [...l].sort((a, b) => a.name.localeCompare(b.name, 'fr'))
      : f.sort === 'date' ? [...l].sort((a, b) => (keyDate(a) ?? '9999').localeCompare(keyDate(b) ?? '9999'))
      : [...l].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
    return l;
  });
  const groups = $derived(STATUS_ORDER.map((s) => ({ s, items: list.filter((p) => p.status === s) })).filter((g) => g.items.length));
  const filters = $derived(f.st.length + f.regie.length);
  /** Téléphone : les filtres restent repliés derrière « Filtres (n) ». */
  let filtersOpen = $state(false);
  const toggle = <T,>(arr: T[], v: T) => (arr.includes(v) ? arr.filter((x) => x !== v) : [...arr, v]);
  const clients = $derived([...new Set(projects.map((p) => p.client).filter(Boolean))] as string[]);

  // Import d'un fichier portable (section 13.2) : aperçu, puis Remplacer ou Copie.
  let imp = $state<{ file: File; name: string; exists: boolean; counts: Record<string, number>; catalog: number; warnings: string[] } | null>(null);
  let importing = $state(false);
  async function previewImport(f: File) {
    try { const r = await api.upload<any>('/projects/import?mode=preview', f, 'application/octet-stream'); imp = { file: f, ...r }; } catch (e) { fail(e); }
  }
  async function runImport(mode: 'replace' | 'copy') {
    if (!imp) return; importing = true;
    try { const r = await api.upload<{ project_id: string; catalog_added: number; warnings: string[] }>(`/projects/import?mode=${mode}`, imp.file, 'application/octet-stream'); imp = null; navigate(`/projets/${r.project_id}`); }
    catch (e) { fail(e); } finally { importing = false; }
  }
</script>

<div class="page">
  <div class="page-head">
    <div><p class="eyebrow">{projects.filter((p) => !['done', 'cancelled'].includes(p.status)).length} projets actifs sur {projects.length}</p><h1 class="display">Projets</h1></div>
    {#if can('projets', 'ecriture')}<div class="head-actions"><button type="button" class="btn" onclick={(e) => (e.currentTarget.nextElementSibling as HTMLInputElement).click()}><Icon name="upload" />Importer</button><input type="file" accept=".strata" hidden onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; (e.target as HTMLInputElement).value = ''; if (f) previewImport(f); }}><button class="btn primary" onclick={() => (wizard = true)}><Icon name="plus" />Nouveau projet</button></div>{/if}
  </div>

  <div class="pj-bar">
    <div class="pj-row">
      <label class="search pj-search"><Icon name="search" /><input class="input" bind:value={f.q} placeholder="Nom, client, lieu, type de projet…" aria-label="Rechercher un projet"></label>
      <div class="seg" aria-label="Tri">{#each [['updated', 'Récents'], ['date', 'Date'], ['name', 'A → Z']] as [k, l]}<button class:on={f.sort === k} onclick={() => (f.sort = k as typeof f.sort)}>{l}</button>{/each}</div>
      <div class="seg" aria-label="Affichage"><button class:on={f.view === 'cards'} onclick={() => (f.view = 'cards')} title="Cartes"><Icon name="grid" size="sm" /></button><button class:on={f.view === 'list'} onclick={() => (f.view = 'list')} title="Liste"><Icon name="list" size="sm" /></button></div>
    </div>
    <button class="btn sm pj-ftoggle" aria-expanded={filtersOpen} aria-controls="pj-filters" onclick={() => (filtersOpen = !filtersOpen)}><Icon name="sliders" size="sm" />Filtres{filters ? ` (${filters})` : ''}<Icon name="chevd" size="sm" /></button>
    <div class="pj-row pj-filters" id="pj-filters" class:open={filtersOpen}>
      <span class="pj-lbl">Statut</span>
      {#each STATUS_ORDER as s}<button class="fchip" class:on={f.st.includes(s)} style="--c:{STATUS[s].color}" onclick={() => (f.st = toggle(f.st, s))}>{STATUS[s].label} <span class="muted">{projects.filter((p) => p.status === s).length}</span></button>{/each}
      <span class="pj-sep"></span>
      <span class="pj-lbl">Régie</span>
      {#each Object.entries(REGIE) as [k, r]}<button class="fchip cxf" class:on={f.regie.includes(k as Regie)} onclick={() => (f.regie = toggle(f.regie, k as Regie))}>{r.label}</button>{/each}
      {#if filters}<button class="btn ghost sm" onclick={() => { f.st = []; f.regie = []; }}>Effacer {filters} filtre{filters > 1 ? 's' : ''}</button>{/if}
    </div>
  </div>

  {#if loading}<span class="spin"></span>
  {:else if !groups.length}
    <div class="empty">{projects.length ? 'Aucun projet ne correspond à ces filtres.' : 'Aucun projet pour l’instant.'}{#if !projects.length && can('projets', 'ecriture')} <button class="ilink" onclick={() => (wizard = true)}>Créer le premier</button>{/if}</div>
  {:else if f.view === 'list'}
    <section class="card" style="padding-top:6px">
      <div class="tbl-wrap"><table class="tbl pj-tbl">
        <thead><tr><th>Projet</th><th>Statut</th><th>Régie</th><th>Client · lieu</th><th>Exploitation</th><th class="num">Échéance</th><th>Fiche</th></tr></thead>
        <tbody>
          {#each list as p (p.id)}
            {@const kd = keyDate(p)}
            {@const d = kd ? daysBetween(today, kd) : null}
            {@const act = !['done', 'cancelled'].includes(p.status)}
            <tr class="click" onclick={() => navigate(`/projets/${p.id}`)}>
              <td><a href="/projets/{p.id}" class="pj-name" onclick={(e) => e.stopPropagation()}>{p.name}</a>{#if p.service_type}<span class="muted small">&nbsp;· {p.service_type}</span>{/if}</td>
              <td><StatusChip status={p.status} /></td>
              <td>{REGIE[p.regie].label}</td>
              <td class="pj-cl">{p.client ?? 'Projet interne'}{p.venue ? ` · ${p.venue}` : ''}</td>
              <td class="tnum">{period(p)}</td>
              <td class="num">{#if act && d != null && d >= 0}<span class="tally" style="--c:{urgency(d)}">J-{d}</span>{:else if act && d != null}<span class="muted small">passée</span>{:else}<span class="muted small">—</span>{/if}</td>
              <td style="width:150px"><Vu value={p.completion} n={16} cls={p.completion >= 80 ? 'ok' : ''} /> <span class="muted small tnum">{p.completion} %</span></td>
            </tr>
          {/each}
        </tbody>
      </table></div>
    </section>
  {:else}
    <div class="pj-groups">
      {#each groups as g (g.s)}
        <section class="pj-group">
          <div class="group-h"><StatusChip status={g.s} /><span class="count">{g.items.length}</span></div>
          <div class="proj-grid">
            {#each g.items as p (p.id)}
              {@const kd = keyDate(p)}
              {@const d = kd ? daysBetween(today, kd) : null}
              {@const act = !['done', 'cancelled'].includes(p.status)}
              <a class="pcard" class:off={!act} href="/projets/{p.id}" style="--c:{STATUS[p.status].color};text-decoration:none">
                <span class="pc-vis"><Ill kind={REGIE[p.regie].ill} /><span class="pc-tag lbl-t">{REGIE[p.regie].label}</span>
                  <span class="pc-j">{#if act && d != null && d >= 0}<span class="tally" style="--c:{urgency(d)}">J-{d}</span>{:else if !act}<span class="tally off">{p.status === 'done' ? 'Livré' : 'Annulé'}</span>{/if}</span></span>
                <span class="pc-body">{#if p.service_type}<span class="lbl-t">{p.service_type}</span>{/if}<b class="pc-name">{p.name}</b><span class="pc-sub">{p.client ?? 'Projet interne'}{p.venue ? ` · ${p.venue}` : ''}</span>
                  <span class="pc-foot"><span><span class="lbl-t">Exploitation</span><b>{kd ? fdate(kd, { day: 'numeric', month: 'short', year: 'numeric' }) : 'à fixer'}</b></span><span><span class="lbl-t">Fiche · {p.completion} %</span><Vu value={p.completion} n={20} cls={p.completion >= 80 ? 'ok' : ''} /></span></span></span>
              </a>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {/if}
</div>

{#if imp}
  <Dialog title="Importer « {imp.name} »" eyebrow="Projet" onclose={() => (imp = null)}>
    <p class="small">{Object.entries(imp.counts).filter(([k]) => ['containers', 'articles', 'instances', 'cables', 'drawings', 'plan_maps', 'net_vlans'].includes(k)).map(([k, v]) => `${v} ${({ containers: 'élément(s)', articles: 'article(s)', instances: 'exemplaire(s)', cables: 'câble(s)', drawings: 'page(s) de dessin', plan_maps: 'carte(s)', net_vlans: 'VLAN' } as Record<string, string>)[k]}`).join(' · ')}</p>
    <p class="small">{imp.catalog} fiche(s) du catalogue jointe(s) : seules les manquantes sont ajoutées, aucune fiche existante n’est modifiée.</p>
    {#each imp.warnings as w}<p class="small" style="color:var(--warn,#B78400)">{w}</p>{/each}
    {#if imp.exists}<p class="small"><b>Ce projet existe déjà ici.</b> Remplacer le remet dans l’état du fichier (l’état actuel est gardé dans un point de restauration) ; Copie crée un nouveau projet « (copie) ».</p>{/if}
    {#snippet footer()}<button class="btn" onclick={() => (imp = null)}>Annuler</button>{#if imp?.exists}<button class="btn" disabled={importing} onclick={() => runImport('replace')}>Remplacer</button>{/if}<button class="btn primary" disabled={importing} onclick={() => runImport(imp?.exists ? 'copy' : 'replace')}>{imp?.exists ? 'Copie' : 'Importer'}</button>{/snippet}
  </Dialog>
{/if}
{#if wizard}<Wizard {clients} onclose={() => (wizard = false)} />{/if}

<style>
  .pj-bar { display: flex; flex-direction: column; gap: 10px; padding: 12px 14px; border-radius: 12px; background: var(--surface); box-shadow: 0 0 0 1px var(--line), 0 1px 2px rgba(0, 0, 0, .04); }
  .pj-row { display: flex; align-items: center; gap: 8px 10px; flex-wrap: wrap; min-width: 0; }
  .pj-search { flex: 1 1 320px; max-width: none; }
  .pj-lbl { font: 600 10px/1 var(--mono); letter-spacing: .09em; text-transform: uppercase; color: var(--fg-3); margin-right: 2px; }
  .pj-sep { width: 1px; height: 20px; background: var(--line); margin: 0 4px; }
  .fchip .muted { font: 600 11px var(--mono); }
  .pj-ftoggle { display: none; align-self: flex-start; }
  .pj-ftoggle[aria-expanded='true'] :global(.ic:last-child) { transform: rotate(180deg); }
  @media (max-width: 640px) {
    .pj-ftoggle { display: inline-flex; }
    .pj-filters:not(.open) { display: none; }
  }
  .pj-groups { display: flex; flex-direction: column; gap: 28px; }
  .pj-group { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .pj-group :global(.group-h .chip.st) { font: 650 17px/1 var(--display); }
  .pj-group .proj-grid { grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 14px; }
  .pj-group :global(.pcard) { border-radius: 12px; box-shadow: 0 0 0 1px var(--line), 0 1px 2px rgba(0, 0, 0, .04); }
  .pj-group :global(.pc-vis) { height: 128px; padding-top: 30px; }
  .pj-group :global(.pc-tag), .pj-group :global(.pc-j) { z-index: 1; top: 8px; }
  .pj-group :global(.pc-tag) { left: 10px; padding: 4px 7px; border-radius: 6px; background: color-mix(in srgb, var(--surface) 88%, transparent); box-shadow: 0 0 0 1px var(--line); }
  .pj-group :global(.pc-vis .ill) { width: auto; max-width: min(78%, 220px); max-height: 92px; }
  .pj-group :global(.pc-name) { font-size: 17px; }
  .pj-group :global(.pc-body) { padding: 12px 14px 14px; gap: 3px; }
  .pj-group :global(.pc-sub) { font-size: 12.5px; }
  .pj-group :global(.pc-foot) { margin-top: 8px; padding-top: 10px; gap: 14px; }
  .pj-group :global(.pc-foot b) { font-size: 13px; }
  .pj-name { font-weight: 650; color: var(--fg); text-decoration: none; }
  .pj-name:hover { color: var(--accent); }
  .pj-cl { color: var(--fg-2); max-width: 340px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pj-tbl td { white-space: nowrap; }
</style>
