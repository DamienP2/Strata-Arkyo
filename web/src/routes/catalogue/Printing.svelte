<script lang="ts">
  import { ask } from '../../lib/ask.svelte';
  import Confirm from '../../components/Confirm.svelte';
  // Bibliothèque d'impression (sections 6.4 et 11.7) : cartouches, planches d'étiquettes,
  // modèles de dossier et de colonnes, logo du site. Partagés par tout le site.
  import Icon from '../../components/Icon.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { forgetTitleBlocks, PAPER_MM, type LabelSheet, type TitleBlock, type Zone } from '../../lib/pdfkit';

  interface DocDef { key: string; group: string; title: string; columns: string[] }
  interface DocOpt { key: string; include: boolean; paper: string | null; landscape: boolean | null; columns: string[]; hidden: string[]; title: string | null }
  interface DT { id: string; name: string; description: string | null; docs: DocOpt[] }
  interface CM { id: string; table: string; name: string; columns: string[] }

  const canSet = $derived(can('parametres', 'ecriture'));
  const canProj = $derived(can('projets', 'ecriture'));
  let tab = $state<'tb' | 'ls' | 'dt' | 'cm' | 'logo'>('tb');
  let tbs = $state<TitleBlock[]>([]);
  let lss = $state<LabelSheet[]>([]);
  let dts = $state<DT[]>([]);
  let cms = $state<CM[]>([]);
  let defs = $state<{ docs: DocDef[]; variables: { key: string; label: string }[] }>({ docs: [], variables: [] });
  let logo = $state<string | null>(null);
  let dirtyTb = $state(false), dirtyLs = $state(false);

  async function load() {
    try {
      const [a, b, c, d, e, s] = await Promise.all([
        api.get<{ items: TitleBlock[] }>('/print/title-blocks'), api.get<{ items: LabelSheet[] }>('/print/label-sheets'),
        api.get<{ items: DT[] }>('/print/dossier-templates'), api.get<{ items: CM[] }>('/print/column-models'),
        api.get<typeof defs>('/print/docs'), api.get<{ logo: string | null }>('/site'),
      ]);
      tbs = a.items; lss = b.items; dts = c.items; cms = d.items; defs = e; logo = s.logo; dirtyTb = false; dirtyLs = false;
    } catch (x) { fail(x); }
  }
  load();

  let errs = $state<ApiError | null>(null);
  async function saveTb() { try { tbs = (await api.put<{ items: TitleBlock[] }>('/print/title-blocks', { items: tbs })).items; dirtyTb = false; errs = null; forgetTitleBlocks(); toast('Cartouches enregistrés.'); } catch (e) { if (e instanceof ApiError && e.fields.length) { errs = e; toast(e.fields[0]!.message, 'err'); } else fail(e); } }
  async function saveLs() { try { lss = (await api.put<{ items: LabelSheet[] }>('/print/label-sheets', { items: lss })).items; dirtyLs = false; errs = null; toast('Planches enregistrées.'); } catch (e) { if (e instanceof ApiError && e.fields.length) { errs = e; toast(e.fields[0]!.message, 'err'); } else fail(e); } }
  async function restore(kind: 'title-blocks' | 'label-sheets') {
    try { const r = await api.post<{ items: any[]; restored: number }>(`/print/${kind}/restore`); if (kind === 'title-blocks') { tbs = r.items; forgetTitleBlocks(); } else lss = r.items; toast(r.restored ? `${r.restored} modèle${r.restored > 1 ? 's' : ''} d’origine réinséré${r.restored > 1 ? 's' : ''}.` : 'Rien à restaurer : tous les modèles d’origine sont présents.'); } catch (e) { fail(e); }
  }
  function move<T>(list: T[], i: number, d: number): T[] { const j = i + d; if (j < 0 || j >= list.length) return list; const l = [...list]; [l[i], l[j]] = [l[j]!, l[i]!]; return l; }

  // ----- Cartouches -----
  let tbEdit = $state<number | null>(null);
  const cur = $derived(tbEdit != null ? tbs[tbEdit] ?? null : null);
  function newTb() { tbs = [...tbs, { key: `cartouche-${tbs.length + 1}`, name: 'Nouveau cartouche', paper: 'A3', landscape: true, width: 160, height: 24, zones: [{ x: 0, y: 0, w: 100, h: 12, kind: 'variable', value: 'title', label: 'Titre', size: 11, bold: true }, { x: 100, y: 0, w: 60, h: 12, kind: 'variable', value: 'date', label: 'Date', size: 8, bold: false }, { x: 0, y: 12, w: 160, h: 12, kind: 'variable', value: 'project', label: 'Projet', size: 8, bold: false }], active: true, builtin: false }]; tbEdit = tbs.length - 1; dirtyTb = true; }
  function dupTb(i: number) { const t = $state.snapshot(tbs[i]!) as TitleBlock; tbs = [...tbs, { ...t, key: `${t.key}-copie`, name: `${t.name} (copie)`, builtin: false }]; dirtyTb = true; }
  function setZone(i: number, p: Partial<Zone>) { if (tbEdit == null) return; tbs[tbEdit]!.zones = tbs[tbEdit]!.zones.map((z, j) => (j === i ? { ...z, ...p } : z)); dirtyTb = true; }
  const SAMPLE: Record<string, string> = { title: 'Synoptique vidéo', project: 'Finale', reference: 'REF-042', client: 'Client', address: 'Arena, Nantes', date: '11/11/2026', sheet: '3', sheets: '12', author: 'C. Martin', checker: 'L. Petit', version: 'B', standards: 'NF C 15-100', notes: 'Pour exécution', site: 'Studio', view: 'Élément', format: 'A3 paysage' };

  async function delTb(i: number) { if (!(await ask({ title: `Retirer le cartouche « ${tbs[i]!.name} » ?`, text: 'Il disparaît de la liste ; la suppression est appliquée à l’enregistrement.', confirmLabel: 'Retirer', danger: true }))) return; tbs = tbs.filter((_, j) => j !== i); dirtyTb = true; }

  // ----- Planches -----
  let lsEdit = $state<number | null>(null);
  const curLs = $derived(lsEdit != null ? lss[lsEdit] ?? null : null);
  function newLs() { lss = [...lss, { key: `planche-${lss.length + 1}`, name: 'Nouvelle planche', reference: `REF-${lss.length + 1}`, brand: null, paper: 'A4', margin_top: 15, margin_left: 10, cols: 4, rows: 10, gap_x: 3, gap_y: 2, label_w: 45, label_h: 24, print_h: 0, diam_min: null, diam_max: null, active: true, builtin: false }]; lsEdit = lss.length - 1; dirtyLs = true; }
  function setLs(p: Partial<LabelSheet>) { if (lsEdit == null) return; lss[lsEdit] = { ...lss[lsEdit]!, ...p }; dirtyLs = true; }

  async function delLs(i: number) { if (!(await ask({ title: `Retirer la planche « ${lss[i]!.name} » ?`, text: 'Elle disparaît de la liste ; la suppression est appliquée à l’enregistrement.', confirmLabel: 'Retirer', danger: true }))) return; lss = lss.filter((_, j) => j !== i); dirtyLs = true; }

  // ----- Modèles de dossier -----
  let dtEdit = $state<DT | null>(null);
  async function saveDt() { if (!dtEdit) return; try { await api.patch('/print/dossier-templates', dtEdit); dtEdit = null; await load(); } catch (e) { fail(e); } }
  async function dupDt(t: DT) { try { await api.post('/print/dossier-templates/duplicate', { id: t.id }); await load(); } catch (e) { fail(e); } }
  async function delDt(t: DT) { if (!(await ask({ title: `Supprimer le modèle « ${t.name} » ?`, confirmLabel: 'Supprimer', danger: true }))) return; try { await api.post('/print/dossier-templates/delete', { id: t.id }); await load(); } catch (e) { fail(e); } }
  async function delCm(c: CM) { if (!(await ask({ title: `Supprimer le modèle de colonnes « ${c.name} » ?`, confirmLabel: 'Supprimer', danger: true }))) return; try { await api.post('/print/column-models/delete', { id: c.id }); await load(); } catch (e) { fail(e); } }
  const docTitle = (k: string) => defs.docs.find((d) => d.key === k)?.title ?? k;

  // ----- Logo -----
  async function uploadLogo(f: File | undefined) {
    if (!f) return;
    try { const r = await api.putRaw<{ logo: string }>('/site/logo', f, f.type); logo = r.logo; forgetTitleBlocks(); toast('Logo enregistré.'); } catch (e) { fail(e); }
  }
  let logoConfirm = $state(false);
  async function dropLogo() { logoConfirm = false; try { await api.del('/site/logo'); logo = null; forgetTitleBlocks(); toast('Logo retiré.'); } catch (e) { fail(e); } }
</script>

<div class="seg pr-tabs" role="tablist" aria-label="Impression">
  {#each [['tb', 'Cartouches'], ['ls', 'Planches d’étiquettes'], ['dt', 'Modèles de dossier'], ['cm', 'Modèles de colonnes'], ['logo', 'Logo du site']] as [k, l]}<button role="tab" aria-selected={tab === k} class:on={tab === k} onclick={() => (tab = k as typeof tab)}>{l}</button>{/each}
</div>

{#if tab === 'tb'}
  <div class="pr-bar"><span class="muted small">Cartouches des exports PDF ; le premier actif sert par défaut. Les modèles d’origine se restaurent sans toucher aux vôtres.</span><span style="flex:1"></span>
    {#if canSet}<button class="btn sm ghost" onclick={() => restore('title-blocks')}>Restaurer les modèles d’origine</button><button class="btn sm" onclick={newTb}><Icon name="plus" size="sm" />Cartouche</button><button class="btn sm primary" disabled={!dirtyTb} onclick={saveTb}>Enregistrer</button>{/if}
  </div>
  <div class="grid">
    {#each tbs as t, i (t.key + i)}
      <article class="card it" class:off={!t.active}>
        <div class="it-h"><b>{t.name}</b>{#if t.builtin}<span class="chip sm" title="Modèle d’origine, livré avec l’application">par défaut</span>{/if}</div>
        <svg class="tbp" viewBox="-1 -1 {Math.max(t.width, 10) + 2} {Math.max(t.height, 6) + 2}" aria-label="Aperçu du cartouche {t.name}">
          {#if !t.width}<text x="1" y="4" font-size="3" fill="currentColor" opacity=".5">Sans cartouche</text>{/if}
          <rect width={t.width} height={t.height} fill="none" stroke="currentColor" stroke-width=".4" />
          {#each t.zones as z}<rect x={z.x} y={z.y} width={z.w} height={z.h} fill="none" stroke="currentColor" stroke-width=".2" />{#if z.label}<text x={z.x + 1} y={z.y + 2.4} font-size="1.8" fill="currentColor" opacity=".6">{z.label.toUpperCase()}</text>{/if}<text x={z.x + 1} y={z.y + z.h - 1.4} font-size={Math.min(z.size * 0.35, z.h * 0.6)} font-weight={z.bold ? 700 : 400} fill="currentColor">{z.kind === 'logo' || z.value === 'logo' ? '[logo]' : z.kind === 'text' ? z.value : (SAMPLE[z.value] ?? z.value)}</text>{/each}
        </svg>
        <span class="muted small">{t.paper} {t.landscape ? 'paysage' : 'portrait'}{t.width && t.height ? ` · ${t.width} × ${t.height} mm` : ' · aucun cartouche imprimé'}</span>
        {#if canSet}
          <div class="it-a">
            <label class="toggle-row small"><input type="checkbox" checked={t.active} onchange={(e) => { tbs[i]!.active = (e.target as HTMLInputElement).checked; dirtyTb = true; }}>Actif</label><span style="flex:1"></span>
            <button class="icon-btn sm up" title="Monter" aria-label="Monter {t.name}" disabled={i === 0} onclick={() => { tbs = move(tbs, i, -1); dirtyTb = true; }}><Icon name="chevd" size="sm" /></button>
            <button class="icon-btn sm" title="Descendre" aria-label="Descendre {t.name}" disabled={i === tbs.length - 1} onclick={() => { tbs = move(tbs, i, 1); dirtyTb = true; }}><Icon name="chevd" size="sm" /></button>
            <button class="icon-btn sm" title="Dupliquer" aria-label="Dupliquer {t.name}" onclick={() => dupTb(i)}><Icon name="copy" size="sm" /></button>
            <button class="icon-btn sm" title="Modifier" aria-label="Modifier {t.name}" onclick={() => (tbEdit = i)}><Icon name="pen" size="sm" /></button>
            <button class="icon-btn sm del" title="Supprimer…" aria-label="Supprimer {t.name}" onclick={() => delTb(i)}><Icon name="trash" size="sm" /></button>
          </div>
        {/if}
      </article>
    {/each}
  </div>
{:else if tab === 'ls'}
  <div class="pr-bar"><span class="muted small">Planches pour les étiquettes de câbles : la zone transparente s’enroule autour du câble.</span><span style="flex:1"></span>
    {#if canSet}<button class="btn sm ghost" onclick={() => restore('label-sheets')}>Restaurer les modèles d’origine</button><button class="btn sm" onclick={newLs}><Icon name="plus" size="sm" />Planche</button><button class="btn sm primary" disabled={!dirtyLs} onclick={saveLs}>Enregistrer</button>{/if}
  </div>
  <div class="grid">
    {#each lss as s, i (s.key + i)}
      {@const pw = PAPER_MM[s.paper]?.[0] ?? 210}{@const ph = PAPER_MM[s.paper]?.[1] ?? 297}
      <article class="card it" class:off={!s.active}>
        <div class="it-h"><b>{s.name}</b><span class="mono muted small">{s.reference}</span>{#if s.brand}<span class="muted small">{s.brand}</span>{/if}</div>
        <svg class="lsp" viewBox="0 0 {pw} {ph}" aria-label="Aperçu de la planche {s.name}">
          <rect width={pw} height={ph} fill="#fff" stroke="#c9ced6" stroke-width="1" />
          {#each Array.from({ length: s.rows }) as _, r}{#each Array.from({ length: s.cols }) as __, c}
            {@const x = s.margin_left + c * (s.label_w + s.gap_x)}{@const y = s.margin_top + r * (s.label_h + s.gap_y)}
            <rect {x} {y} width={s.label_w} height={s.label_h} fill="#EEF1F5" stroke="#9aa3ae" stroke-width=".5" rx="1" />
            {#if s.print_h}<rect {x} {y} width={s.label_w} height={s.print_h} fill="#D9E3FF" rx="1" />{/if}
          {/each}{/each}
        </svg>
        <span class="muted small">{s.paper} · {s.cols} × {s.rows} · {s.label_w} × {s.label_h} mm{s.diam_min ? ` · Ø ${s.diam_min}–${s.diam_max} mm` : ''}</span>
        {#if canSet}
          <div class="it-a">
            <label class="toggle-row small"><input type="checkbox" checked={s.active} onchange={(e) => { lss[i]!.active = (e.target as HTMLInputElement).checked; dirtyLs = true; }}>Active</label><span style="flex:1"></span>
            <button class="icon-btn sm up" title="Monter" aria-label="Monter {s.name}" disabled={i === 0} onclick={() => { lss = move(lss, i, -1); dirtyLs = true; }}><Icon name="chevd" size="sm" /></button>
            <button class="icon-btn sm" title="Descendre" aria-label="Descendre {s.name}" disabled={i === lss.length - 1} onclick={() => { lss = move(lss, i, 1); dirtyLs = true; }}><Icon name="chevd" size="sm" /></button>
            <span class="slot" aria-hidden="true"></span>
            <button class="icon-btn sm" title="Modifier" aria-label="Modifier {s.name}" onclick={() => (lsEdit = i)}><Icon name="pen" size="sm" /></button>
            <button class="icon-btn sm del" title="Supprimer…" aria-label="Supprimer {s.name}" onclick={() => delLs(i)}><Icon name="trash" size="sm" /></button>
          </div>
        {/if}
      </article>
    {/each}
  </div>
{:else if tab === 'dt'}
  <div class="pr-bar"><span class="muted small">Compositions de dossier enregistrées depuis l’assistant de dossier d’un projet. Elles ne contiennent aucune donnée de projet.</span></div>
  {#if !dts.length}<EmptyState icon="file" message="Aucun modèle de dossier." hint="Créez-en un à la fin d’un export de dossier, depuis un projet." />{/if}
  <div class="grid">
    {#each dts as t (t.id)}
      <article class="card it">
        <div class="it-h"><b>{t.name}</b></div>
        {#if t.description}<p class="small muted" style="margin:0">{t.description}</p>{/if}
        <p class="small" style="margin:0">{t.docs.filter((d) => d.include).map((d) => d.title ?? docTitle(d.key)).join(' · ')}</p>
        {#if canProj}<div class="it-a"><span style="flex:1"></span><button class="icon-btn sm" title="Dupliquer" aria-label="Dupliquer {t.name}" onclick={() => dupDt(t)}><Icon name="copy" size="sm" /></button><button class="icon-btn sm" title="Modifier" aria-label="Modifier {t.name}" onclick={() => (dtEdit = $state.snapshot(t) as DT)}><Icon name="pen" size="sm" /></button><button class="icon-btn sm del" title="Supprimer…" aria-label="Supprimer {t.name}" onclick={() => delDt(t)}><Icon name="trash" size="sm" /></button></div>{/if}
      </article>
    {/each}
  </div>
{:else if tab === 'cm'}
  <div class="pr-bar"><span class="muted small">Sélection et ordre des colonnes des tableaux (adressage IP, 2110, équipements, préparation), enregistrés depuis l’assistant de dossier.</span></div>
  {#if !cms.length}<EmptyState icon="list" message="Aucun modèle de colonnes." hint="Ils s’enregistrent depuis l’assistant de dossier d’un projet." />{/if}
  {#each defs.docs.filter((d) => cms.some((c) => c.table === d.key)) as d}
    <h3 class="h3">{d.title}</h3>
    {#each cms.filter((c) => c.table === d.key) as c (c.id)}<div class="card cm"><b>{c.name}</b><span class="small muted">{c.columns.join(' · ')}</span><span style="flex:1"></span>{#if canProj}<button class="icon-btn sm del" title="Supprimer…" aria-label="Supprimer {c.name}" onclick={() => delCm(c)}><Icon name="trash" size="sm" /></button>{/if}</div>{/each}
  {/each}
{:else}
  <section class="card logo">
    <div class="lg-prev">{#if logo}<img src={logo} alt="Logo du site">{:else}<span class="muted">Aucun logo</span>{/if}</div>
    <p class="small muted">Imprimé dans les cartouches (zone logo) et sur la page de garde du dossier. PNG ou JPG, 512 Ko au plus.</p>
    {#if canSet}<div class="row"><button type="button" class="btn sm" onclick={(e) => (e.currentTarget.nextElementSibling as HTMLInputElement).click()}><Icon name="upload" size="sm" />Choisir une image</button><input type="file" accept="image/png,image/jpeg" hidden onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; (e.target as HTMLInputElement).value = ''; uploadLogo(f); }}>{#if logo}<button class="btn sm ghost danger act-danger" onclick={() => (logoConfirm = true)}>Retirer le logo…</button>{/if}</div>{/if}
  </section>
{/if}

{#if cur && tbEdit != null}
  <Dialog title={cur.name} eyebrow="Cartouche" xl onclose={() => (tbEdit = null)}>
    <div class="fg4">
      <label class="field">Nom<input class="input" value={cur.name} maxlength="60" oninput={(e) => { tbs[tbEdit!]!.name = (e.target as HTMLInputElement).value; dirtyTb = true; }}></label>
      <label class="field">Clé<input class="input mono" value={cur.key} maxlength="40" oninput={(e) => { tbs[tbEdit!]!.key = (e.target as HTMLInputElement).value; dirtyTb = true; }}></label>
      <label class="field">Papier<select class="input" value={cur.paper} onchange={(e) => { tbs[tbEdit!]!.paper = (e.target as HTMLSelectElement).value; dirtyTb = true; }}>{#each Object.keys(PAPER_MM) as p}<option value={p}>{p}</option>{/each}</select></label>
      <label class="field">Orientation<select class="input" value={cur.landscape ? 'l' : 'p'} onchange={(e) => { tbs[tbEdit!]!.landscape = (e.target as HTMLSelectElement).value === 'l'; dirtyTb = true; }}><option value="l">Paysage</option><option value="p">Portrait</option></select></label>
      <label class="field">Largeur (mm)<input class="input" type="number" min="0" max="400" value={cur.width} onchange={(e) => { tbs[tbEdit!]!.width = Number((e.target as HTMLInputElement).value); dirtyTb = true; }}></label>
      <label class="field">Hauteur (mm)<input class="input" type="number" min="0" max="200" value={cur.height} onchange={(e) => { tbs[tbEdit!]!.height = Number((e.target as HTMLInputElement).value); dirtyTb = true; }}></label>
    </div>
    <svg class="tbp big" viewBox="-1 -1 {Math.max(cur.width, 10) + 2} {Math.max(cur.height, 6) + 2}">
      <rect width={cur.width} height={cur.height} fill="none" stroke="currentColor" stroke-width=".4" />
      {#each cur.zones as z}<rect x={z.x} y={z.y} width={z.w} height={z.h} fill="none" stroke="currentColor" stroke-width=".2" />{#if z.label}<text x={z.x + 1} y={z.y + 2.4} font-size="1.8" fill="currentColor" opacity=".6">{z.label.toUpperCase()}</text>{/if}<text x={z.x + 1} y={z.y + z.h - 1.4} font-size={Math.min(z.size * 0.35, z.h * 0.6)} font-weight={z.bold ? 700 : 400} fill="currentColor">{z.kind === 'logo' || z.value === 'logo' ? '[logo]' : z.kind === 'text' ? z.value : (SAMPLE[z.value] ?? z.value)}</text>{/each}
    </svg>
    <table class="tbl zt"><thead><tr><th>Nature</th><th>Valeur</th><th>Étiquette</th><th>X</th><th>Y</th><th>L</th><th>H</th><th>Taille</th><th>Gras</th><th></th></tr></thead><tbody>
      {#each cur.zones as z, i}
        <tr>
          <td><select class="input sm" value={z.kind} onchange={(e) => setZone(i, { kind: (e.target as HTMLSelectElement).value as Zone['kind'], value: (e.target as HTMLSelectElement).value === 'variable' ? 'title' : (e.target as HTMLSelectElement).value === 'logo' ? 'logo' : '' })}><option value="variable">Variable</option><option value="text">Texte</option><option value="logo">Logo</option></select></td>
          <td>{#if z.kind === 'variable'}<select class="input sm" value={z.value} onchange={(e) => setZone(i, { value: (e.target as HTMLSelectElement).value })}>{#each defs.variables as v}<option value={v.key}>{v.label}</option>{/each}</select>{:else if z.kind === 'text'}<input class="input sm" value={z.value} maxlength="200" oninput={(e) => setZone(i, { value: (e.target as HTMLInputElement).value })}>{:else}<span class="muted small">Logo du site</span>{/if}</td>
          <td><input class="input sm" value={z.label ?? ''} oninput={(e) => setZone(i, { label: (e.target as HTMLInputElement).value || null })}></td>
          {#each ['x', 'y', 'w', 'h', 'size'] as f}<td><input class="input sm num" type="number" step="0.5" value={(z as any)[f]} onchange={(e) => setZone(i, { [f]: Number((e.target as HTMLInputElement).value) } as Partial<Zone>)}></td>{/each}
          <td><input type="checkbox" checked={z.bold} onchange={(e) => setZone(i, { bold: (e.target as HTMLInputElement).checked })} aria-label="Gras"></td>
          <td><button class="icon-btn sm" aria-label="Retirer la zone" onclick={() => { tbs[tbEdit!]!.zones = cur.zones.filter((_, j) => j !== i); dirtyTb = true; }}><Icon name="x" size="sm" /></button></td>
        </tr>
      {/each}
    </tbody></table>
    <button class="btn sm" onclick={() => { tbs[tbEdit!]!.zones = [...cur.zones, { x: 0, y: 0, w: Math.min(40, cur.width || 40), h: Math.min(8, cur.height || 8), kind: 'variable', value: 'notes', label: 'Notes', size: 8, bold: false }]; dirtyTb = true; }}><Icon name="plus" size="sm" />Zone</button>
    {#snippet footer()}<button class="btn" onclick={() => (tbEdit = null)}>Fermer</button><button class="btn primary" onclick={async () => { await saveTb(); if (!errs) tbEdit = null; }}>Enregistrer</button>{/snippet}
  </Dialog>
{/if}

{#if curLs && lsEdit != null}
  <Dialog title={curLs.name} eyebrow="Planche d’étiquettes" wide onclose={() => (lsEdit = null)}>
    <div class="fg3">
      <label class="field">Nom<input class="input" value={curLs.name} maxlength="80" oninput={(e) => setLs({ name: (e.target as HTMLInputElement).value })}></label>
      <label class="field">Référence<input class="input mono" value={curLs.reference} maxlength="40" oninput={(e) => setLs({ reference: (e.target as HTMLInputElement).value })}></label>
      <label class="field">Marque<input class="input" value={curLs.brand ?? ''} oninput={(e) => setLs({ brand: (e.target as HTMLInputElement).value || null })}></label>
      <label class="field">Papier<select class="input" value={curLs.paper} onchange={(e) => setLs({ paper: (e.target as HTMLSelectElement).value as LabelSheet['paper'] })}>{#each Object.keys(PAPER_MM) as p}<option value={p}>{p}</option>{/each}</select></label>
      {#each [['cols', 'Colonnes'], ['rows', 'Lignes'], ['label_w', 'Largeur (mm)'], ['label_h', 'Hauteur (mm)'], ['print_h', 'Zone imprimable (mm, 0 = tout)'], ['margin_top', 'Marge haute'], ['margin_left', 'Marge gauche'], ['gap_x', 'Espacement horizontal'], ['gap_y', 'Espacement vertical'], ['diam_min', 'Ø câble min. (mm)'], ['diam_max', 'Ø câble max. (mm)']] as [f, l]}
        <label class="field">{l}<input class="input" type="number" step="0.1" value={(curLs as any)[f] ?? ''} onchange={(e) => { const v = (e.target as HTMLInputElement).value; setLs({ [f]: v === '' && f.startsWith('diam') ? null : Number(v) } as Partial<LabelSheet>); }}></label>
      {/each}
    </div>
    {#snippet footer()}<button class="btn" onclick={() => (lsEdit = null)}>Fermer</button><button class="btn primary" onclick={async () => { await saveLs(); if (!errs) lsEdit = null; }}>Enregistrer</button>{/snippet}
  </Dialog>
{/if}

{#if dtEdit}
  <Dialog title={dtEdit.name} eyebrow="Modèle de dossier" onclose={() => (dtEdit = null)}>
    <label class="field">Nom<input class="input" bind:value={dtEdit.name} maxlength="100"></label>
    <label class="field">Description<textarea class="input" rows="3" maxlength="500" value={dtEdit.description ?? ''} oninput={(e) => (dtEdit!.description = (e.target as HTMLTextAreaElement).value || null)}></textarea></label>
    <div class="docs">{#each dtEdit.docs as d}<label class="toggle-row small"><input type="checkbox" bind:checked={d.include}>{d.title ?? docTitle(d.key)}</label>{/each}</div>
    {#snippet footer()}<button class="btn" onclick={() => (dtEdit = null)}>Annuler</button><button class="btn primary" onclick={saveDt}>Enregistrer</button>{/snippet}
  </Dialog>
{/if}

{#if logoConfirm}
  <Confirm title="Retirer le logo du site ?" confirmLabel="Retirer le logo" danger onconfirm={dropLogo} onclose={() => (logoConfirm = false)}>
    <p>Il disparaît des cartouches de tous les documents. Vous pourrez en choisir un autre.</p>
  </Confirm>
{/if}

<style>
  .pr-tabs { margin-bottom: 14px; flex-wrap: wrap; }
  .pr-bar { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; margin-bottom: 12px; }
  .pr-bar > .muted { max-width: 72ch; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 12px; }
  .it { padding: 14px; display: grid; gap: 8px; }
  .it.off { opacity: .55; }
  .it-h { display: flex; gap: 8px; align-items: baseline; flex-wrap: wrap; }
  .it-a { display: flex; gap: 4px; align-items: center; flex-wrap: nowrap; padding-top: 8px; border-top: 1px solid var(--line); }
  .it-a .up :global(.ic) { transform: rotate(180deg); }
  .it-a .del { margin-left: 12px; color: var(--err); }
  .it-a .slot { width: 30px; flex: none; }
  .tbp { width: 100%; height: 90px; color: var(--fg); background: var(--surface-2); border-radius: 8px; padding: 6px; font-family: Helvetica, Arial, sans-serif; }
  .tbp.big { height: 180px; margin: 10px 0; }
  .lsp { width: 100%; height: 220px; background: var(--surface-2); border-radius: 8px; padding: 6px; }
  .chip.sm { font-size: 11px; padding: 1px 6px; border-radius: 99px; background: var(--surface-2); font-weight: 600; }
  .zt .input.sm { height: 32px; min-width: 0; }
  .zt .num { width: 64px; }
  .zt td { padding: 3px 4px; }
  .field { display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
  .fg4 { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; }
  .h3 { font-size: 16px; margin: 16px 0 8px; }
  .cm { padding: 10px 14px; display: flex; gap: 10px; align-items: center; margin-bottom: 6px; flex-wrap: wrap; }
  .logo { padding: 18px; display: grid; gap: 10px; justify-items: start; }
  .lg-prev { width: 220px; height: 110px; border: 1px dashed var(--line); border-radius: 12px; display: grid; place-items: center; background: #fff; }
  .lg-prev img { max-width: 200px; max-height: 96px; }
  .row { display: flex; gap: 8px; }
  .docs { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 4px; }
</style>
