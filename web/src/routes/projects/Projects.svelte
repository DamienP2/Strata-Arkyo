<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import PageHead from '../../components/PageHead.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
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
  const REGIE_ICON: Record<Regie, string> = { car: 'truck', flight_case: 'case', fixed: 'monitor' };
  /** « Projet interne » n'informe que si d'autres projets ont un client. */
  const anyClient = $derived(projects.some((p) => p.client));
  const subOf = (p: ProjectSummary) => [p.client ?? (anyClient ? 'Projet interne' : ''), p.venue].filter(Boolean).join(' · ');
  const activeCount = $derived(projects.filter((p) => !['done', 'cancelled'].includes(p.status)).length);
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
  <PageHead title="Projets" count={loading ? '' : `${activeCount} actif${activeCount > 1 ? 's' : ''} sur ${projects.length}`}>
    {#snippet actions()}{#if can('projets', 'ecriture')}<button type="button" class="btn" onclick={(e) => (e.currentTarget.nextElementSibling as HTMLInputElement).click()}><Icon name="upload" />Importer</button><input type="file" accept=".strata" hidden onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; (e.target as HTMLInputElement).value = ''; if (f) previewImport(f); }}>{#if projects.length || loading}<button class="btn primary" onclick={() => (wizard = true)}><Icon name="plus" />Nouveau projet</button>{/if}{/if}{/snippet}
  </PageHead>

  {#if loading || projects.length}
  <div class="pj-bar">
    <div class="pj-row">
      <label class="search pj-search"><Icon name="search" /><input class="input" bind:value={f.q} placeholder="Nom, client, lieu, type de projet…" aria-label="Rechercher un projet"></label>
      <div class="seg" aria-label="Tri">{#each [['updated', 'Récents'], ['date', 'Date'], ['name', 'A → Z']] as [k, l]}<button class:on={f.sort === k} onclick={() => (f.sort = k as typeof f.sort)}>{l}</button>{/each}</div>
      <div class="seg" aria-label="Affichage"><button class:on={f.view === 'cards'} onclick={() => (f.view = 'cards')} title="Cartes"><Icon name="grid" size="sm" /></button><button class:on={f.view === 'list'} onclick={() => (f.view = 'list')} title="Liste"><Icon name="list" size="sm" /></button></div>
    </div>
    <button class="btn sm pj-ftoggle" aria-expanded={filtersOpen} aria-controls="pj-filters" onclick={() => (filtersOpen = !filtersOpen)}><Icon name="sliders" size="sm" />Filtres{filters ? ` (${filters})` : ''}<Icon name="chevd" size="sm" /></button>
    <div class="pj-row pj-filters" id="pj-filters" class:open={filtersOpen}>
      <div class="pj-fg" role="group" aria-labelledby="pj-l-st">
        <span class="pj-lbl" id="pj-l-st">Statut</span>
        {#each STATUS_ORDER as s}{@const n = projects.filter((p) => p.status === s).length}<button class="fchip" class:on={f.st.includes(s)} class:zero={!n} style="--c:{STATUS[s].color}" aria-pressed={f.st.includes(s)} onclick={() => (f.st = toggle(f.st, s))}>{STATUS[s].label} <span class="pj-n">{n}</span></button>{/each}
      </div>
      <div class="pj-fg" role="group" aria-labelledby="pj-l-rg">
        <span class="pj-lbl" id="pj-l-rg">Régie</span>
        {#each Object.entries(REGIE) as [k, r]}{@const n = projects.filter((p) => p.regie === k).length}<button class="fchip cxf" class:on={f.regie.includes(k as Regie)} class:zero={!n} aria-pressed={f.regie.includes(k as Regie)} onclick={() => (f.regie = toggle(f.regie, k as Regie))}><Icon name={REGIE_ICON[k as Regie]} size="sm" />{r.label} <span class="pj-n">{n}</span></button>{/each}
      </div>
      {#if filters}<button class="btn ghost sm" onclick={() => { f.st = []; f.regie = []; }}>Effacer {filters} filtre{filters > 1 ? 's' : ''}</button>{/if}
    </div>
  </div>
  {/if}

  {#if loading}<span class="spin"></span>
  {:else if !groups.length}
    {#if projects.length}<EmptyState icon="search" message="Aucun projet ne correspond à ces critères." hint="Modifiez la recherche ou effacez les filtres." />
    {:else}<EmptyState icon="folder" message="Aucun projet pour l’instant." hint="L’assistant vous guide étape par étape.">{#snippet action()}{#if can('projets', 'ecriture')}<button class="btn primary" onclick={() => (wizard = true)}><Icon name="plus" />Créer le premier projet</button>{/if}{/snippet}</EmptyState>{/if}
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
              <td class="pj-cl" title={subOf(p)}>{subOf(p) || '—'}</td>
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
              <a class="pcard pj-card" class:off={!act} href="/projets/{p.id}" style="--c:{STATUS[p.status].color};text-decoration:none">
                <span class="pj-top"><span class="pj-ico" title={REGIE[p.regie].label}><Icon name={REGIE_ICON[p.regie]} size="sm" /></span><span class="pj-kind">{REGIE[p.regie].label}{#if p.service_type} · {p.service_type}{/if}</span>
                  <span class="pc-j">{#if act && d != null && d >= 0}<span class="tally" style="--c:{urgency(d)}">J-{d}</span>{:else if !act}<span class="tally off">{p.status === 'done' ? 'Livré' : 'Annulé'}</span>{/if}</span></span>
                <span class="pc-body"><b class="pc-name" title={p.name}>{p.name}</b>{#if subOf(p)}<span class="pc-sub" title={subOf(p)}>{subOf(p)}</span>{/if}
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
  .pj-lbl { font: 600 12.5px/1 var(--font); color: var(--fg-2); margin-right: 4px; }
  .pj-fg { display: flex; align-items: center; gap: 6px 8px; flex-wrap: wrap; padding: 4px 10px 4px 0; }
  .pj-fg + .pj-fg { padding-left: 14px; border-left: 1px solid var(--line-2); }
  .pj-n { font: 600 12px var(--font); font-variant-numeric: tabular-nums; color: var(--fg-3); }
  .fchip.on .pj-n { color: inherit; }
  .pj-filters { gap: 8px 6px; }
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
  .pj-card { border-top: 3px solid var(--c); }
  .pj-top { display: flex; align-items: center; gap: 9px; padding: 12px 14px 0; min-width: 0; }
  .pj-ico { flex: none; display: grid; place-items: center; width: 30px; height: 30px; border-radius: 8px; background: color-mix(in srgb, var(--c) 14%, var(--surface)); color: var(--c); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 30%, transparent); }
  .pj-kind { flex: 1; min-width: 0; font-size: 12.5px; font-weight: 600; color: var(--fg-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pj-top .pc-j { position: static; flex: none; }
  .pcard.off .pj-ico { filter: grayscale(1); opacity: .6; }
  .pj-group :global(.pc-name), .pj-group :global(.pc-sub) { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; white-space: normal; }
  .pj-group :global(.pc-name) { font-size: 17px; line-height: 1.2; }
  .pj-group :global(.pc-body) { flex: 1; padding: 10px 14px 14px; gap: 3px; }
  .pj-group :global(.pc-sub) + :global(.pc-foot), .pj-group :global(.pc-name) + :global(.pc-foot) { margin-top: auto; }
  .pj-group :global(.pc-sub) { font-size: 13px; }
  .pj-group :global(.pc-foot) { margin-top: 10px; padding-top: 10px; gap: 14px; }
  .pj-group :global(.pc-foot b) { font-size: 13px; }
  .pj-name { font-weight: 650; color: var(--fg); text-decoration: none; }
  .pj-name:hover { color: var(--accent); }
  .pj-cl { color: var(--fg-2); max-width: 340px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pj-tbl td { white-space: nowrap; }
</style>
