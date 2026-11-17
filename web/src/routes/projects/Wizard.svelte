<script lang="ts">
  import { tick } from 'svelte';
  import { modal } from '../../lib/modal';
  // Assistant de création : le type, le nom, puis les dates. Le reste (client, lieu,
  // emplacements, éléments) se renseigne ensuite dans le projet.
  import Icon from '../../components/Icon.svelte';
  import Ill from '../../components/Ill.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import { api, ApiError } from '../../lib/api';
  import { navigate } from '../../lib/router.svelte';
  import { toast } from '../../lib/toast.svelte';
  import { PROJECT_TYPES, type ProjectType } from '../../lib/projects';

  let { onclose }: { onclose: () => void; clients?: string[] } = $props();
  const today = new Date().toISOString().slice(0, 10);

  let d = $state({ type: 'prestation' as ProjectType, name: '', planning_start: today, planning_end: '', shoot_start: '', shoot_end: '' });
  let step = $state(0);
  let busy = $state(false);
  let errors = $state<Record<string, string>>({});
  let taken = $state(false);
  let timer: ReturnType<typeof setTimeout>;

  const STEPS = ['type', 'name', 'dates'] as const;
  type Step = (typeof STEPS)[number];
  const cur = $derived(STEPS[step]!);
  const presta = $derived(d.type === 'prestation');
  const FIELD_STEP: Record<string, Step> = { name: 'name', planning_start: 'dates', planning_end: 'dates', shoot_start: 'dates', shoot_end: 'dates' };

  function checkName() {
    clearTimeout(timer);
    const n = d.name.trim();
    if (!n) { taken = false; return; }
    timer = setTimeout(async () => { taken = (await api.get<{ taken: boolean }>(`/projects/check-name?name=${encodeURIComponent(n)}`)).taken; }, 250);
  }

  /** Erreurs bloquantes de l'étape courante, avant d'aller plus loin. */
  function stepError(s: Step): Record<string, string> {
    const e: Record<string, string> = {};
    if (s === 'name') { if (!d.name.trim()) e.name = 'Le nom est obligatoire.'; else if (taken) e.name = 'Un projet porte déjà ce nom.'; }
    if (s === 'dates') {
      if (!d.planning_start || !d.planning_end) e.planning_end = 'Le début et la fin de la préparation sont obligatoires.';
      else if (d.planning_start < today) e.planning_start = 'La date est passée.';
      else if (d.planning_end < d.planning_start) e.planning_end = 'La fin précède le début.';
      if (presta) {
        if (!d.shoot_start || !d.shoot_end) e.shoot_end = 'Le début et la fin de l’exploitation sont obligatoires.';
        else if (d.shoot_start < today) e.shoot_start = 'La date est passée.';
        else if (d.shoot_end < d.shoot_start) e.shoot_end = 'La fin précède le début.';
      }
    }
    return e;
  }
  function next() {
    errors = stepError(cur);
    if (Object.keys(errors).length) return;
    if (cur === 'dates') { create(); return; }
    step = Math.min(step + 1, STEPS.length - 1);
  }
  async function create() {
    busy = true; errors = {};
    const body = {
      name: d.name, regie: PROJECT_TYPES[d.type].regie,
      planning_start: d.planning_start, planning_end: d.planning_end,
      shoot_start: presta ? d.shoot_start || null : null, shoot_end: presta ? d.shoot_end || null : null,
    };
    try {
      const r = await api.post<{ project: { id: string } }>('/projects', body);
      toast('Projet créé.');
      onclose();
      navigate(`/projets/${r.project.id}`);
    } catch (e) {
      if (e instanceof ApiError && e.fields.length) {
        errors = Object.fromEntries(e.fields.map((f) => [f.field, f.message]));
        step = STEPS.indexOf(FIELD_STEP[e.fields[0]!.field] ?? 'dates');
      } else errors = { general: e instanceof Error ? e.message : 'Création impossible.' };
    } finally { busy = false; }
  }
  // Fermeture (croix, Échap, fond) : on ne demande que si la saisie a commencé, comme Dialog.
  const initial = JSON.stringify($state.snapshot(d));
  const dirty = () => JSON.stringify(d) !== initial;
  let asking = $state(false);
  let askEl = $state<HTMLButtonElement>();
  $effect(() => { if (asking) askEl?.focus(); });
  function requestClose() { if (busy) return; if (dirty()) asking = true; else onclose(); }
  async function resume() { asking = false; await tick(); document.querySelector<HTMLElement>('.wz-body :is(input,button.choice)')?.focus(); }
  /** Champ en erreur : aria-invalid et aria-describedby vers le message (FieldError `for`). */
  const inv = (key: string, id: string) => ({ 'aria-invalid': errors[key] ? true : undefined, 'aria-describedby': errors[key] ? `${id}-err` : undefined });
</script>

<div class="modal" role="presentation" use:modal={{ close: onclose, locked: () => busy, dirty, ask: () => (asking ? resume() : (asking = true)) }}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Nouveau projet">
    <div class="wz-head">
      <div class="row"><span class="eyebrow">Nouveau projet · étape {step + 1} sur {STEPS.length}</span><button type="button" class="icon-btn sm" disabled={busy} onclick={requestClose} aria-label="Fermer (Échap)" title="Fermer (Échap)"><Icon name="x" /></button></div>
      <div class="bar"><i style="width:{(100 * (step + 1)) / STEPS.length}%"></i></div>
    </div>
    <form class="wz-body" onsubmit={(e) => { e.preventDefault(); next(); }}>
      {#if cur === 'type'}
        <h2>Quel type de projet ?</h2>
        <div class="choices">
          {#each Object.entries(PROJECT_TYPES) as [k, t]}
            <button type="button" class="choice" class:on={d.type === k} aria-pressed={d.type === k} onclick={() => (d.type = k as ProjectType)}>
              <span class="ch-vis"><Ill kind={t.ill} /></span><b>{t.label}</b><span class="muted small">{t.help}</span>
            </button>
          {/each}
        </div>
      {:else if cur === 'name'}
        <h2>Comment s’appelle-t-il ?</h2>
        <div class="field"><label for="wz-name">Nom du projet</label><input class="input" id="wz-name" bind:value={d.name} oninput={checkName} maxlength="120" autocomplete="off" placeholder={presta ? 'Ex. Finale Coupe Atlantique' : 'Ex. Studio 3'} {...inv('name', 'wz-name')}>
          {#if taken}<span class="help err">Un projet porte déjà ce nom.</span>{:else if d.name.trim()}<span class="help ok">Nom disponible.</span>{/if}<FieldError message={errors.name} for="wz-name" /></div>
      {:else}
        <h2>{presta ? 'Quelles dates ?' : 'Période de préparation'}</h2>
        <fieldset class="wz-period">
          <legend class="eyebrow">Préparation</legend>
          <p class="muted small">Conception, commandes, câblage en atelier{presta ? '' : ', mise en service'}.</p>
          <div class="form-grid">
            <div class="field"><label for="wz-ps">Début</label><input class="input" type="date" id="wz-ps" bind:value={d.planning_start} min={today} {...inv('planning_start', 'wz-ps')}><FieldError message={errors.planning_start} for="wz-ps" /></div>
            <div class="field"><label for="wz-pe">Fin</label><input class="input" type="date" id="wz-pe" bind:value={d.planning_end} min={d.planning_start || today} {...inv('planning_end', 'wz-pe')}><FieldError message={errors.planning_end} for="wz-pe" /></div>
          </div>
        </fieldset>
        {#if presta}
          <fieldset class="wz-period">
            <legend class="eyebrow">Exploitation</legend>
            <button type="button" class="ux3-info" title="Tournage ou événement : un seul jour si début et fin sont identiques." aria-label="Tournage ou événement : un seul jour si début et fin sont identiques."><Icon name="info" size="sm" /></button>
            <div class="form-grid">
              <div class="field"><label for="wz-ss">Début</label><input class="input" type="date" id="wz-ss" bind:value={d.shoot_start} min={d.planning_start || today} {...inv('shoot_start', 'wz-ss')}><FieldError message={errors.shoot_start} for="wz-ss" /></div>
              <div class="field"><label for="wz-se">Fin</label><input class="input" type="date" id="wz-se" bind:value={d.shoot_end} min={d.shoot_start || today} {...inv('shoot_end', 'wz-se')}><FieldError message={errors.shoot_end} for="wz-se" /></div>
            </div>
          </fieldset>
        {/if}
        {#if errors.general}<div class="alert err">{errors.general}</div>{/if}
      {/if}
      <button type="submit" hidden aria-hidden="true" tabindex="-1"></button>
    </form>
    {#if asking}
      <div class="wz-foot wz-ask" role="alertdialog" aria-label="Fermer sans enregistrer ?">
        <span>Fermer sans enregistrer ? Ce qui n’a pas été enregistré sera perdu.</span>
        <button type="button" class="btn ghost" bind:this={askEl} onclick={resume}>Continuer la saisie</button>
        <button type="button" class="btn primary danger" onclick={onclose}>Fermer sans enregistrer</button>
      </div>
    {:else}
      <div class="wz-foot">
        <button type="button" class="btn ghost" onclick={() => (step = Math.max(0, step - 1))} disabled={step === 0 || busy}><Icon name="chevl" size="sm" />Retour</button>
        {#if cur === 'dates'}<button type="button" class="btn primary" onclick={next} disabled={busy}>{#if busy}<span class="spin" aria-hidden="true"></span>{/if}Créer le projet</button>
        {:else}<button type="button" class="btn primary" onclick={next}>Continuer<Icon name="chevr" size="sm" /></button>{/if}
      </div>
    {/if}
  </div>
</div>

<style>
  /* Demande de fermeture : même pied que Dialog (texte, « Continuer la saisie », danger plein en taille M). */
  .wz-ask { gap: 8px; align-items: center; }
  .wz-ask span { flex: 1; font-size: var(--fs-md); color: var(--fg-2); }
  .wz-period { border: 0; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .wz-period + .wz-period { margin-top: 14px; }
  .wz-period legend { padding: 0; margin-bottom: 2px; }
  .wz-period p { margin: 0 0 4px; }
  .choice .small { font-weight: 400; text-align: center; }
</style>
