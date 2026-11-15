<script lang="ts">
  import { ask } from '../../lib/ask.svelte';
  import Confirm from '../../components/Confirm.svelte';
  // Bibliothèque d'impression (sections 6.4 et 11.7) : cartouches, planches d'étiquettes,
  // modèles de dossier et de colonnes, logo du site. Partagés par tout le site.
  import Icon from '../../components/Icon.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Seg from '../../components/Seg.svelte';
  import NumField from '../../components/NumField.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { registerLeaveGuard } from '../../lib/router.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { forgetTitleBlocks, PAPER_MM, type LabelSheet, type TitleBlock, type Zone } from '../../lib/pdfkit';

  interface DocDef { key: string; group: string; title: string; columns: string[] }
  interface DocOpt { key: string; include: boolean; paper: string | null; landscape: boolean | null; columns: string[]; hidden: string[]; title: string | null }
  interface DT { id: string; name: string; description: string | null; docs: DocOpt[] }
  interface CM { id: string; table: string; name: string; columns: string[] }

  const canSet = $derived(can('parametres', 'ecriture'));
  const canProj = $derived(can('projets', 'ecriture'));
  type Tab = 'tb' | 'ls' | 'dt' | 'cm' | 'logo';
  let tab = $state<Tab>('tb');
  let tbs = $state<TitleBlock[]>([]);
  let lss = $state<LabelSheet[]>([]);
  let dts = $state<DT[]>([]);
  let cms = $state<CM[]>([]);
  let defs = $state<{ docs: DocDef[]; variables: { key: string; label: string }[] }>({ docs: [], variables: [] });
  let logo = $state<string | null>(null);
  let dirtyTb = $state(false), dirtyLs = $state(false);
  let savingTb = $state(false), savingLs = $state(false), uploading = $state(false);
  // Quitter la page avec des cartouches ou des planches non enregistrés : on demande.
  $effect(() => registerLeaveGuard(() => dirtyTb || dirtyLs));

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

  async function saveTb() {
    savingTb = true;
    try { tbs = (await api.put<{ items: TitleBlock[] }>('/print/title-blocks', { items: tbs })).items; dirtyTb = false; forgetTitleBlocks(); toast('Cartouches enregistrés.'); }
    catch (e) { if (e instanceof ApiError && e.fields.length) toast(e.fields[0]!.message, 'err'); else fail(e); }
    finally { savingTb = false; }
  }
  async function saveLs() {
    savingLs = true;
    try { lss = (await api.put<{ items: LabelSheet[] }>('/print/label-sheets', { items: lss })).items; dirtyLs = false; toast('Planches enregistrées.'); }
    catch (e) { if (e instanceof ApiError && e.fields.length) toast(e.fields[0]!.message, 'err'); else fail(e); }
    finally { savingLs = false; }
  }
  async function restore(kind: 'title-blocks' | 'label-sheets') {
    try { const r = await api.post<{ items: any[]; restored: number }>(`/print/${kind}/restore`); if (kind === 'title-blocks') { tbs = r.items; forgetTitleBlocks(); } else lss = r.items; toast(r.restored ? `${r.restored} modèle${r.restored > 1 ? 's' : ''} d’origine réinséré${r.restored > 1 ? 's' : ''}.` : 'Rien à restaurer : tous les modèles d’origine sont présents.'); } catch (e) { fail(e); }
  }
  function move<T>(list: T[], i: number, d: number): T[] { const j = i + d; if (j < 0 || j >= list.length) return list; const l = [...list]; [l[i], l[j]] = [l[j]!, l[i]!]; return l; }

  // ----- Cartouches -----
  /** Le cartouche utilisé par défaut : le premier actif de la liste. */
  const defaultTb = $derived(tbs.findIndex((t) => t.active));
  function makeDefault(i: number) { const l = [...tbs]; const [t] = l.splice(i, 1); l.unshift({ ...t!, active: true }); tbs = l; dirtyTb = true; }
  // Fenêtre d'édition : une copie, appliquée à la liste par « Appliquer », jetée par « Annuler ».
  let tbDraft = $state<{ i: number | null; t: TitleBlock; orig: string } | null>(null);
  function editTb(i: number) { const t = structuredClone($state.snapshot(tbs[i]!)) as TitleBlock; tbDraft = { i, t, orig: JSON.stringify(t) }; }
  function newTb() {
    const t: TitleBlock = { key: `cartouche-${tbs.length + 1}`, name: 'Nouveau cartouche', paper: 'A3', landscape: true, width: 160, height: 24, zones: [{ x: 0, y: 0, w: 100, h: 12, kind: 'variable', value: 'title', label: 'Titre', size: 11, bold: true }, { x: 100, y: 0, w: 60, h: 12, kind: 'variable', value: 'date', label: 'Date', size: 8, bold: false }, { x: 0, y: 12, w: 160, h: 12, kind: 'variable', value: 'project', label: 'Projet', size: 8, bold: false }], active: true, builtin: false };
    tbDraft = { i: null, t, orig: JSON.stringify(t) };
  }
  function applyTb() {
    if (!tbDraft) return;
    const t = $state.snapshot(tbDraft.t) as TitleBlock;
    tbs = tbDraft.i == null ? [...tbs, t] : tbs.map((x, j) => (j === tbDraft!.i ? t : x));
    dirtyTb = true; tbDraft = null;
  }
  function dupTb(i: number) { const t = $state.snapshot(tbs[i]!) as TitleBlock; tbs = [...tbs, { ...t, key: `${t.key}-copie`, name: `${t.name} (copie)`, builtin: false }]; dirtyTb = true; toast(`« ${t.name} » dupliqué.`); }
  function setZone(i: number, p: Partial<Zone>) { if (!tbDraft) return; tbDraft.t.zones = tbDraft.t.zones.map((z, j) => (j === i ? { ...z, ...p } : z)); }
  const SAMPLE: Record<string, string> = { title: 'Synoptique vidéo', project: 'Finale', reference: 'REF-042', client: 'Client', address: 'Arena, Nantes', date: '11/11/2026', sheet: '3', sheets: '12', author: 'C. Martin', checker: 'L. Petit', version: 'B', standards: 'NF C 15-100', notes: 'Pour exécution', site: 'Studio', view: 'Élément', format: 'A3 paysage' };

  async function delTb(i: number) {
    const t = tbs[i]!;
    if (!(await ask({ title: `Supprimer le cartouche « ${t.name} » ?`, text: 'Il disparaît de la liste ; la suppression est appliquée à l’enregistrement.', confirmLabel: 'Supprimer', danger: true }))) return;
    const before = tbs, wasDirty = dirtyTb;
    tbs = tbs.filter((_, j) => j !== i); dirtyTb = true;
    toast(`« ${t.name} » supprimé.`, 'ok', { run: () => { tbs = before; dirtyTb = wasDirty; } });
  }

  // ----- Planches -----
  let lsDraft = $state<{ i: number | null; s: LabelSheet; orig: string } | null>(null);
  function editLs(i: number) { const s = structuredClone($state.snapshot(lss[i]!)) as LabelSheet; lsDraft = { i, s, orig: JSON.stringify(s) }; }
  function newLs() {
    const s: LabelSheet = { key: `planche-${lss.length + 1}`, name: 'Nouvelle planche', reference: `REF-${lss.length + 1}`, brand: null, paper: 'A4', margin_top: 15, margin_left: 10, cols: 4, rows: 10, gap_x: 3, gap_y: 2, label_w: 45, label_h: 24, print_h: 0, diam_min: null, diam_max: null, active: true, builtin: false };
    lsDraft = { i: null, s, orig: JSON.stringify(s) };
  }
  function applyLs() {
    if (!lsDraft) return;
    const s = $state.snapshot(lsDraft.s) as LabelSheet;
    lss = lsDraft.i == null ? [...lss, s] : lss.map((x, j) => (j === lsDraft!.i ? s : x));
    dirtyLs = true; lsDraft = null;
  }
  async function delLs(i: number) {
    const s = lss[i]!;
    if (!(await ask({ title: `Supprimer la planche « ${s.name} » ?`, text: 'Elle disparaît de la liste ; la suppression est appliquée à l’enregistrement.', confirmLabel: 'Supprimer', danger: true }))) return;
    const before = lss, wasDirty = dirtyLs;
    lss = lss.filter((_, j) => j !== i); dirtyLs = true;
    toast(`« ${s.name} » supprimée.`, 'ok', { run: () => { lss = before; dirtyLs = wasDirty; } });
  }
  /** Champs numériques de la planche : [champ, libellé, unité, vide permis]. */
  const LS_NUM: [keyof LabelSheet, string, string, boolean][] = [
    ['cols', 'Colonnes', '', false], ['rows', 'Lignes', '', false], ['label_w', 'Largeur de l’étiquette', 'mm', false], ['label_h', 'Hauteur de l’étiquette', 'mm', false],
    ['print_h', 'Zone imprimable', 'mm', false], ['margin_top', 'Marge haute', 'mm', false], ['margin_left', 'Marge gauche', 'mm', false],
    ['gap_x', 'Espacement horizontal', 'mm', false], ['gap_y', 'Espacement vertical', 'mm', false], ['diam_min', 'Ø câble minimal', 'mm', true], ['diam_max', 'Ø câble maximal', 'mm', true],
  ];

  // ----- Modèles de dossier -----
  let dtEdit = $state<DT | null>(null);
  let dtOrig = '';
  function editDt(t: DT) { dtEdit = structuredClone($state.snapshot(t)) as DT; dtOrig = JSON.stringify(dtEdit); }
  async function saveDt() { if (!dtEdit) return; const n = dtEdit.name; await api.patch('/print/dossier-templates', dtEdit); dtEdit = null; toast(`« ${n} » enregistré.`); await load(); }
  async function dupDt(t: DT) { try { await api.post('/print/dossier-templates/duplicate', { id: t.id }); toast(`« ${t.name} » dupliqué.`); await load(); } catch (e) { fail(e); } }
  async function delDt(t: DT) {
    if (!(await ask({ title: `Supprimer le modèle « ${t.name} » ?`, confirmLabel: 'Supprimer', danger: true, run: () => api.post('/print/dossier-templates/delete', { id: t.id }) }))) return;
    toast(`« ${t.name} » supprimé.`); await load();
  }
  async function delCm(c: CM) {
    if (!(await ask({ title: `Supprimer le modèle de colonnes « ${c.name} » ?`, confirmLabel: 'Supprimer', danger: true, run: () => api.post('/print/column-models/delete', { id: c.id }) }))) return;
    toast(`« ${c.name} » supprimé.`); await load();
  }
  const docTitle = (k: string) => defs.docs.find((d) => d.key === k)?.title ?? k;

  // ----- Logo -----
  async function uploadLogo(f: File | undefined) {
    if (!f) return;
    uploading = true;
    try { const r = await api.putRaw<{ logo: string }>('/site/logo', f, f.type); logo = r.logo; forgetTitleBlocks(); toast('Logo enregistré.'); } catch (e) { fail(e); } finally { uploading = false; }
  }
  let logoConfirm = $state(false);
  async function dropLogo() { logoConfirm = false; try { await api.del('/site/logo'); logo = null; forgetTitleBlocks(); toast('Logo retiré.'); } catch (e) { fail(e); } }
</script>

{#snippet tbPreview(t: TitleBlock, big: boolean)}
  <svg class="tbp" class:big viewBox="-1 -1 {Math.max(t.width, 10) + 2} {Math.max(t.height, 6) + 2}" role="img" aria-label="Aperçu du cartouche {t.name}">
    {#if !t.width}<text x="1" y="4" font-size="3" fill="currentColor" opacity=".5">Sans cartouche</text>{/if}
    <rect width={t.width} height={t.height} fill="none" stroke="currentColor" stroke-width=".4" />
    {#each t.zones as z}<rect x={z.x} y={z.y} width={z.w} height={z.h} fill="none" stroke="currentColor" stroke-width=".2" />{#if z.label}<text x={z.x + 1} y={z.y + 2.4} font-size="1.8" fill="currentColor" opacity=".6">{z.label.toUpperCase()}</text>{/if}<text x={z.x + 1} y={z.y + z.h - 1.4} font-size={Math.min(z.size * 0.35, z.h * 0.6)} font-weight={z.bold ? 700 : 400} fill="currentColor">{z.kind === 'logo' || z.value === 'logo' ? '[logo]' : z.kind === 'text' ? z.value : (SAMPLE[z.value] ?? z.value)}</text>{/each}
  </svg>
{/snippet}

<div class="pr-tabs">
  <Seg mode="tabs" label="Impression" bind:value={tab} options={[{ value: 'tb', label: 'Cartouches' }, { value: 'ls', label: 'Planches d’étiquettes' }, { value: 'dt', label: 'Modèles de dossier' }, { value: 'cm', label: 'Modèles de colonnes' }, { value: 'logo', label: 'Logo du site' }]} />
</div>

{#if tab === 'tb'}
  <div class="pr-bar"><span class="muted small">Cartouches des exports PDF : le premier actif est utilisé par défaut. Les modèles d’origine se restaurent sans toucher aux vôtres.</span><span style="flex:1"></span>
    {#if canSet}
      {#if dirtyTb}<span class="small muted" role="status">Modifications non enregistrées.</span>{/if}
      <button class="btn sm ghost" onclick={() => restore('title-blocks')}>Restaurer les modèles d’origine</button><button class="btn sm" onclick={newTb}><Icon name="plus" size="sm" />Nouveau cartouche</button><button class="btn sm primary" disabled={!dirtyTb || savingTb} onclick={saveTb}>{#if savingTb}<span class="spin" aria-hidden="true"></span>{/if}Enregistrer</button>
    {/if}
  </div>
  <div class="grid">
    {#each tbs as t, i (t.key + i)}
      <article class="card it" class:off={!t.active}>
        <div class="it-h"><b>{t.name}</b>{#if i === defaultTb}<span class="chip accent" title="Sert sur les PDF quand aucun cartouche n’est choisi">Utilisé par défaut</span>{/if}{#if t.builtin}<span class="chip" title="Modèle livré avec l’application">d’origine</span>{/if}</div>
        {@render tbPreview(t, false)}
        <span class="muted small">{t.paper} {t.landscape ? 'paysage' : 'portrait'}{t.width && t.height ? ` · ${t.width} × ${t.height} mm` : ' · aucun cartouche imprimé'}</span>
        {#if canSet}
          <div class="it-a">
            <label class="toggle-row small"><input type="checkbox" checked={t.active} onchange={(e) => { tbs[i]!.active = (e.target as HTMLInputElement).checked; dirtyTb = true; }}>Actif</label>
            {#if i !== defaultTb}<button class="btn sm ghost" onclick={() => makeDefault(i)}>Définir par défaut</button>{/if}<span style="flex:1"></span>
            <button class="icon-btn sm up" title="Monter {t.name}" aria-label="Monter {t.name}" disabled={i === 0} onclick={() => { tbs = move(tbs, i, -1); dirtyTb = true; }}><Icon name="chevd" size="sm" /></button>
            <button class="icon-btn sm" title="Descendre {t.name}" aria-label="Descendre {t.name}" disabled={i === tbs.length - 1} onclick={() => { tbs = move(tbs, i, 1); dirtyTb = true; }}><Icon name="chevd" size="sm" /></button>
            <button class="icon-btn sm" title="Dupliquer {t.name}" aria-label="Dupliquer {t.name}" onclick={() => dupTb(i)}><Icon name="copy" size="sm" /></button>
            <button class="icon-btn sm" title="Modifier {t.name}" aria-label="Modifier {t.name}" onclick={() => editTb(i)}><Icon name="pen" size="sm" /></button>
            <button class="icon-btn sm danger act-danger" title="Supprimer {t.name}" aria-label="Supprimer {t.name}" onclick={() => delTb(i)}><Icon name="trash" size="sm" /></button>
          </div>
        {/if}
      </article>
    {/each}
  </div>
{:else if tab === 'ls'}
  <div class="pr-bar"><span class="muted small">Planches pour les étiquettes de câbles : la zone transparente s’enroule autour du câble.</span><span style="flex:1"></span>
    {#if canSet}
      {#if dirtyLs}<span class="small muted" role="status">Modifications non enregistrées.</span>{/if}
      <button class="btn sm ghost" onclick={() => restore('label-sheets')}>Restaurer les modèles d’origine</button><button class="btn sm" onclick={newLs}><Icon name="plus" size="sm" />Nouvelle planche</button><button class="btn sm primary" disabled={!dirtyLs || savingLs} onclick={saveLs}>{#if savingLs}<span class="spin" aria-hidden="true"></span>{/if}Enregistrer</button>
    {/if}
  </div>
  <div class="grid">
    {#each lss as s, i (s.key + i)}
      {@const pw = PAPER_MM[s.paper]?.[0] ?? 210}{@const ph = PAPER_MM[s.paper]?.[1] ?? 297}
      <article class="card it" class:off={!s.active}>
        <div class="it-h"><b>{s.name}</b><span class="mono muted small">{s.reference}</span>{#if s.brand}<span class="muted small">{s.brand}</span>{/if}{#if s.builtin}<span class="chip" title="Modèle livré avec l’application">d’origine</span>{/if}</div>
        <svg class="lsp" viewBox="0 0 {pw} {ph}" role="img" aria-label="Aperçu de la planche {s.name}">
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
            <button class="icon-btn sm up" title="Monter {s.name}" aria-label="Monter {s.name}" disabled={i === 0} onclick={() => { lss = move(lss, i, -1); dirtyLs = true; }}><Icon name="chevd" size="sm" /></button>
            <button class="icon-btn sm" title="Descendre {s.name}" aria-label="Descendre {s.name}" disabled={i === lss.length - 1} onclick={() => { lss = move(lss, i, 1); dirtyLs = true; }}><Icon name="chevd" size="sm" /></button>
            <span class="slot" aria-hidden="true"></span>
            <button class="icon-btn sm" title="Modifier {s.name}" aria-label="Modifier {s.name}" onclick={() => editLs(i)}><Icon name="pen" size="sm" /></button>
            <button class="icon-btn sm danger act-danger" title="Supprimer {s.name}" aria-label="Supprimer {s.name}" onclick={() => delLs(i)}><Icon name="trash" size="sm" /></button>
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
        {#if canProj}<div class="it-a"><span style="flex:1"></span><button class="icon-btn sm" title="Dupliquer {t.name}" aria-label="Dupliquer {t.name}" onclick={() => dupDt(t)}><Icon name="copy" size="sm" /></button><button class="icon-btn sm" title="Modifier {t.name}" aria-label="Modifier {t.name}" onclick={() => editDt(t)}><Icon name="pen" size="sm" /></button><button class="icon-btn sm danger act-danger" title="Supprimer {t.name}" aria-label="Supprimer {t.name}" onclick={() => delDt(t)}><Icon name="trash" size="sm" /></button></div>{/if}
      </article>
    {/each}
  </div>
{:else if tab === 'cm'}
  <div class="pr-bar"><span class="muted small">Sélection et ordre des colonnes des tableaux (adressage IP, 2110, équipements, préparation), enregistrés depuis l’assistant de dossier.</span></div>
  {#if !cms.length}<EmptyState icon="list" message="Aucun modèle de colonnes." hint="Ils s’enregistrent depuis l’assistant de dossier d’un projet." />{/if}
  {#each defs.docs.filter((d) => cms.some((c) => c.table === d.key)) as d}
    <h3 class="h3">{d.title}</h3>
    {#each cms.filter((c) => c.table === d.key) as c (c.id)}<div class="card compact cm"><b>{c.name}</b><span class="small muted">{c.columns.join(' · ')}</span><span style="flex:1"></span>{#if canProj}<button class="icon-btn sm danger" title="Supprimer {c.name}" aria-label="Supprimer {c.name}" onclick={() => delCm(c)}><Icon name="trash" size="sm" /></button>{/if}</div>{/each}
  {/each}
{:else}
  <section class="card logo">
    <div class="lg-prev">{#if logo}<img src={logo} alt="Logo du site">{:else}<span class="muted">Aucun logo</span>{/if}</div>
    <p class="small muted">Imprimé dans les cartouches (zone logo) et sur la page de garde du dossier. PNG ou JPG, 512 Ko au plus.</p>
    {#if canSet}<div class="row">{#if logo}<button class="btn sm ghost danger" onclick={() => (logoConfirm = true)}>Retirer le logo…</button>{/if}<button type="button" class="btn sm" disabled={uploading} onclick={(e) => (e.currentTarget.nextElementSibling as HTMLInputElement).click()}>{#if uploading}<span class="spin" aria-hidden="true"></span>{:else}<Icon name="upload" size="sm" />{/if}Choisir une image</button><input type="file" accept="image/png,image/jpeg" hidden onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; (e.target as HTMLInputElement).value = ''; uploadLogo(f); }}></div>{/if}
  </section>
{/if}

{#if tbDraft}
  {@const d = tbDraft}
  <Dialog title={d.t.name || 'Cartouche'} eyebrow="Cartouche" xl onclose={() => (tbDraft = null)} onsubmit={applyTb} submitLabel="Appliquer" dirty={() => JSON.stringify(d.t) !== d.orig}>
    <div class="fg3">
      <div class="field"><label for="tb-n">Nom</label><input class="input" id="tb-n" bind:value={d.t.name} maxlength="60" required></div>
      <div class="field"><label for="tb-k">Clé</label><input class="input mono" id="tb-k" bind:value={d.t.key} maxlength="40" required></div>
      <div class="field"><label for="tb-p">Papier</label><select class="input" id="tb-p" bind:value={d.t.paper}>{#each Object.keys(PAPER_MM) as p}<option value={p}>{p}</option>{/each}</select></div>
      <div class="field"><label for="tb-o">Orientation</label><select class="input" id="tb-o" value={d.t.landscape ? 'l' : 'p'} onchange={(e) => (d.t.landscape = (e.target as HTMLSelectElement).value === 'l')}><option value="l">Paysage</option><option value="p">Portrait</option></select></div>
      <NumField id="tb-w" label="Largeur" unit="mm" min={0} max={400} bind:value={() => d.t.width, (v) => (d.t.width = v ?? 0)} />
      <NumField id="tb-h" label="Hauteur" unit="mm" min={0} max={200} bind:value={() => d.t.height, (v) => (d.t.height = v ?? 0)} />
    </div>
    {@render tbPreview(d.t, true)}
    <p class="help" style="margin:0">Zones : position et taille en mm depuis le coin haut gauche du cartouche, corps du texte en points.</p>
    <div class="tbl-wrap">
      <table class="tbl dense zt"><thead><tr><th>Nature</th><th>Valeur</th><th>Libellé</th><th class="num" title="Position horizontale, en mm">X</th><th class="num" title="Position verticale, en mm">Y</th><th class="num" title="Largeur, en mm">L</th><th class="num" title="Hauteur, en mm">H</th><th class="num" title="Corps du texte, en points">Taille</th><th>Gras</th><th><span class="sr-only">Actions</span></th></tr></thead><tbody>
        {#each d.t.zones as z, i}
          <tr>
            <td><select class="input sm" aria-label="Nature de la zone {i + 1}" value={z.kind} onchange={(e) => { const k = (e.target as HTMLSelectElement).value as Zone['kind']; setZone(i, { kind: k, value: k === 'variable' ? 'title' : k === 'logo' ? 'logo' : '' }); }}><option value="variable">Variable</option><option value="text">Texte</option><option value="logo">Logo</option></select></td>
            <td>{#if z.kind === 'variable'}<select class="input sm" aria-label="Variable de la zone {i + 1}" value={z.value} onchange={(e) => setZone(i, { value: (e.target as HTMLSelectElement).value })}>{#each defs.variables as v}<option value={v.key}>{v.label}</option>{/each}</select>{:else if z.kind === 'text'}<input class="input sm" aria-label="Texte de la zone {i + 1}" value={z.value} maxlength="200" oninput={(e) => setZone(i, { value: (e.target as HTMLInputElement).value })}>{:else}<span class="muted small">Logo du site</span>{/if}</td>
            <td><input class="input sm" aria-label="Libellé de la zone {i + 1}" value={z.label ?? ''} oninput={(e) => setZone(i, { label: (e.target as HTMLInputElement).value || null })}></td>
            {#each [['x', 'X'], ['y', 'Y'], ['w', 'Largeur'], ['h', 'Hauteur'], ['size', 'Taille']] as [f, l]}<td class="num"><input class="input sm mono znum" type="number" step="0.5" aria-label="{l} de la zone {i + 1}" value={(z as any)[f]} onchange={(e) => setZone(i, { [f]: Number((e.target as HTMLInputElement).value) } as Partial<Zone>)}></td>{/each}
            <td><input type="checkbox" checked={z.bold} onchange={(e) => setZone(i, { bold: (e.target as HTMLInputElement).checked })} aria-label="Zone {i + 1} en gras"></td>
            <td class="acts"><button type="button" class="icon-btn sm danger" title="Retirer la zone {i + 1}" aria-label="Retirer la zone {i + 1}" onclick={() => (d.t.zones = d.t.zones.filter((_, j) => j !== i))}><Icon name="x" size="sm" /></button></td>
          </tr>
        {/each}
      </tbody></table>
    </div>
    <button type="button" class="btn sm" onclick={() => (d.t.zones = [...d.t.zones, { x: 0, y: 0, w: Math.min(40, d.t.width || 40), h: Math.min(8, d.t.height || 8), kind: 'variable', value: 'notes', label: 'Notes', size: 8, bold: false }])}><Icon name="plus" size="sm" />Ajouter une zone</button>
  </Dialog>
{/if}

{#if lsDraft}
  {@const d = lsDraft}
  <Dialog title={d.s.name || 'Planche'} eyebrow="Planche d’étiquettes" wide onclose={() => (lsDraft = null)} onsubmit={applyLs} submitLabel="Appliquer" dirty={() => JSON.stringify(d.s) !== d.orig}>
    <div class="fg3">
      <div class="field"><label for="ls-n">Nom</label><input class="input" id="ls-n" bind:value={d.s.name} maxlength="80" required></div>
      <div class="field"><label for="ls-r">Référence</label><input class="input mono" id="ls-r" bind:value={d.s.reference} maxlength="40"></div>
      <div class="field"><label for="ls-b">Marque (facultatif)</label><input class="input" id="ls-b" value={d.s.brand ?? ''} oninput={(e) => (d.s.brand = (e.target as HTMLInputElement).value || null)}></div>
      <div class="field"><label for="ls-p">Papier</label><select class="input" id="ls-p" bind:value={d.s.paper}>{#each Object.keys(PAPER_MM) as p}<option value={p}>{p}</option>{/each}</select></div>
      {#each LS_NUM as [f, l, u, opt]}
        <NumField id="ls-{f}" label={opt ? `${l} (facultatif)` : l} unit={u} step="0.1" min={0} help={f === 'print_h' ? '0 : toute l’étiquette est imprimable.' : undefined}
          bind:value={() => (d.s[f] as number | null), (v) => ((d.s as any)[f] = v ?? (opt ? null : 0))} />
      {/each}
    </div>
  </Dialog>
{/if}

{#if dtEdit}
  {@const d = dtEdit}
  <Dialog title={d.name || 'Modèle de dossier'} eyebrow="Modèle de dossier" onclose={() => (dtEdit = null)} onsubmit={saveDt} submitLabel="Enregistrer" dirty={() => JSON.stringify(d) !== dtOrig}>
    <div class="stack">
      <div class="field"><label for="dt-n">Nom</label><input class="input" id="dt-n" bind:value={d.name} maxlength="100" required></div>
      <div class="field"><label for="dt-d">Description (facultatif)</label><textarea class="input" id="dt-d" rows="3" maxlength="500" value={d.description ?? ''} oninput={(e) => (d.description = (e.target as HTMLTextAreaElement).value || null)}></textarea></div>
      <div class="field"><span class="flabel">Documents inclus</span><div class="docs">{#each d.docs as doc}<label class="toggle-row small"><input type="checkbox" bind:checked={doc.include}>{doc.title ?? docTitle(doc.key)}</label>{/each}</div></div>
    </div>
  </Dialog>
{/if}

{#if logoConfirm}
  <Confirm title="Retirer le logo du site ?" confirmLabel="Retirer le logo" danger onconfirm={dropLogo} onclose={() => (logoConfirm = false)}>
    <p>Il disparaît des cartouches de tous les documents. Vous pourrez en choisir un autre.</p>
  </Confirm>
{/if}

<style>
  .pr-tabs { margin-bottom: 14px; max-width: 100%; overflow-x: auto; }
  .pr-bar { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; margin-bottom: 12px; }
  .pr-bar > .muted { max-width: 72ch; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 320px), 1fr)); gap: 12px; }
  .it { display: grid; gap: 8px; }
  .it.off { opacity: .55; }
  .it-h { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .it-a { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; padding-top: 8px; border-top: 1px solid var(--line); }
  .it-a .up :global(.ic) { transform: rotate(180deg); }
  .it-a .slot { width: 30px; flex: none; }
  .tbp { width: 100%; height: 90px; color: var(--fg); background: var(--surface-2); border-radius: 8px; padding: 6px; font-family: Helvetica, Arial, sans-serif; }
  .tbp.big { height: 180px; margin: 10px 0; }
  .lsp { width: 100%; height: 220px; background: var(--surface-2); border-radius: 8px; padding: 6px; }
  .zt .znum { width: 72px; text-align: right; }
  .h3 { font-size: var(--fs-lg); margin: 16px 0 8px; }
  .cm { display: flex; gap: 10px; align-items: center; margin-bottom: 6px; flex-wrap: wrap; }
  .logo { display: grid; gap: 10px; justify-items: start; }
  .lg-prev { width: 220px; height: 110px; border: 1px dashed var(--line); border-radius: 12px; display: grid; place-items: center; background: #fff; }
  .lg-prev img { max-width: 200px; max-height: 96px; }
  .row { display: flex; gap: 8px; flex-wrap: wrap; }
  .docs { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 4px; }
</style>
