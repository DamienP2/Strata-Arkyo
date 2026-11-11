<script lang="ts">
  // Numérotation des câbles (section 10.7) : portée, mode, règles (site, élément ou ponctuelles)
  // et aperçu du format.
  import { untrack } from 'svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Icon from '../../components/Icon.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import type { Container, ProjectView } from '../../lib/projects';

  interface Rules { with_rack: boolean; sep1: string; sep2: string; digits: number; template: string | null; top_down: boolean; left_right: boolean }
  let { project, view, selection, onclose, ondone }: { project: ProjectView; view: string; selection: string[]; onclose: () => void; ondone: () => void } = $props();

  const SEPS: [string, string][] = [['', 'Aucun'], ['-', '-'], ['.', '.'], ['_', '_'], ['/', '/'], ['*', '*']];
  type Mode = 'missing' | 'all' | 'selection' | 'manual' | 'clear';
  let mode = $state<Mode>(untrack(() => (selection.length ? 'selection' : 'missing')));
  let scope = $state<'view' | 'project'>('view');
  let first = $state('');
  let site = $state<Rules | null>(null);
  let rules = $state<Rules | null>(null);
  let origin = $state<'element' | 'site'>('site');
  let busy = $state(false);
  let errors = $state<Record<string, string>>({});
  const container = $derived<Container | null>(view.startsWith('container:') ? project.containers.find((c) => c.id === view.slice(10)) ?? null : null);
  const code = $derived(container ? (container.cable_code || container.name.normalize('NFD').replace(/[^A-Za-z0-9]/g, '').slice(0, 4).toUpperCase()) : 'RGA');

  api.get<Rules>('/site/numbering').then((r) => {
    site = r;
    const own = container ? (container as unknown as { numbering?: Rules | null }).numbering : null;
    origin = own ? 'element' : 'site';
    rules = { ...(own ?? r), template: (own ?? r).template ?? '' };
  }, fail);

  function preview(r: Rules, prefix: string, dir: number, seq: number) {
    const s = String(seq).padStart(Math.max(1, r.digits - 1), '0');
    if (r.template) return r.template.replaceAll('{rack}', code).replaceAll('{sep1}', r.sep1).replaceAll('{prefix}', prefix).replaceAll('{dir}', String(dir)).replaceAll('{seq}', s).replaceAll('{sep2}', r.sep2).replaceAll('{suffix}', '');
    return `${r.with_rack ? code + r.sep1 : ''}${prefix}${dir}${s}`;
  }
  const clean = (r: Rules) => ({ ...r, template: r.template?.trim() || null });

  async function saveRules(where: 'site' | 'element') {
    if (!rules) return;
    errors = {};
    try {
      if (where === 'site') { site = await api.put<Rules>('/site/numbering', clean(rules)); toast('Règles du site enregistrées.'); }
      else if (container) {
        const { id, project_id: _p, position: _o, rev, totals: _t, ...data } = container;
        await api.patch(`/containers/${id}`, { ...data, numbering: clean(rules), rev });
        toast(`Règles de ${container.name} enregistrées.`);
        origin = 'element';
      }
    } catch (e) { if (e instanceof ApiError && e.fields.length) errors = Object.fromEntries(e.fields.map((f) => [f.field.replace(/^numbering\./, ''), f.message])); else fail(e); }
  }
  async function run() {
    if (!rules) return;
    busy = true; errors = {};
    try {
      const body: Record<string, unknown> = { mode, view: scope === 'view' ? view : null };
      if (mode === 'selection' || mode === 'manual' || (mode === 'clear' && selection.length)) body.ids = selection;
      if (mode === 'manual') body.first = first;
      if (mode !== 'manual' && mode !== 'clear') body.rules = clean(rules);
      const r = await api.post<{ changed: number }>(`/projects/${project.project.id}/numbering`, body);
      toast(mode === 'clear' ? `${r.changed} numéro(s) effacé(s).` : r.changed ? `${r.changed} câble(s) numéroté(s).` : 'Aucun câble à numéroter.', r.changed ? 'ok' : 'info');
      ondone();
    } catch (e) { if (e instanceof ApiError && e.fields.length) errors = Object.fromEntries(e.fields.map((f) => [f.field, f.message])); else fail(e); }
    finally { busy = false; }
  }
</script>

<Dialog title="Numéroter les câbles" eyebrow={project.project.name} {onclose} wide>
  {#if !rules}<span class="spin"></span>
  {:else}
    <div class="stack">
      <div class="nb-row">
        <div class="field"><span class="flabel">Portée</span><div class="seg"><button class:on={scope === 'view'} onclick={() => (scope = 'view')}>{container ? container.name : 'Cette vue'}</button><button class:on={scope === 'project'} onclick={() => (scope = 'project')}>Tout le projet</button></div></div>
        <div class="field"><span class="flabel">Mode</span><div class="seg wrap">
          {#each [['missing', 'Sans numéro'], ['all', 'Tout renuméroter'], ['selection', `Sélection (${selection.length})`], ['manual', 'Saisie manuelle'], ['clear', 'Effacer']] as [k, l]}
            <button class:on={mode === k} disabled={(k === 'selection' || k === 'manual') && !selection.length} onclick={() => (mode = k as Mode)}>{l}</button>
          {/each}
        </div></div>
      </div>
      {#if mode === 'manual'}
        <div class="field" style="max-width:320px"><label for="nb-first">Premier numéro</label><input class="input mono" id="nb-first" bind:value={first} placeholder="V-0100">{#if errors.first}<span class="ferr">{errors.first}</span>{/if}<span class="help">Les {selection.length} câble(s) choisis reçoivent ce numéro, puis les suivants.</span></div>
      {:else if mode === 'clear'}
        <div class="alert warn"><Icon name="alert" /><span>{selection.length ? `Les numéros des ${selection.length} câble(s) choisis` : `Tous les numéros ${scope === 'view' ? 'de la vue' : 'du projet'}`} seront effacés. Les compteurs ne reculent pas : un numéro n’est jamais réutilisé.</span></div>
      {:else}
        <section class="nb-rules">
          <div class="nb-h"><span class="lbl-t">Règles</span><span class="muted small">{origin === 'element' ? `propres à ${container?.name}` : 'du site'} · modifiables ici pour cette fois</span></div>
          <div class="nb-row">
            <label class="toggle-row"><input type="checkbox" bind:checked={rules.with_rack} disabled={!!rules.template}>Code de l’élément en tête</label>
            <div class="field"><span class="flabel">Séparateur 1</span><div class="seg">{#each SEPS as [v, l]}<button class:on={rules.sep1 === v} disabled={!!rules.template && false} onclick={() => (rules!.sep1 = v)}>{l}</button>{/each}</div></div>
            <div class="field"><span class="flabel">Séparateur 2</span><div class="seg">{#each SEPS as [v, l]}<button class:on={rules.sep2 === v} onclick={() => (rules!.sep2 = v)}>{l}</button>{/each}</div></div>
          </div>
          <div class="nb-row">
            <div class="field" style="max-width:160px"><label for="nb-d">Chiffres (sens compris)</label><input class="input mono" id="nb-d" type="number" min="3" max="10" bind:value={rules.digits}>{#if errors.digits}<span class="ferr">{errors.digits}</span>{/if}</div>
            <div class="field" style="flex:1"><label for="nb-t">Modèle personnalisé</label><input class="input mono" id="nb-t" bind:value={rules.template} placeholder={'Vide : format standard. Ex. {rack}{sep1}{prefix}{dir}{seq}'}>{#if errors.template}<span class="ferr">{errors.template}</span>{/if}</div>
          </div>
          <div class="nb-row">
            <div class="field"><span class="flabel">Ordre vertical</span><div class="seg"><button class:on={rules.top_down} onclick={() => (rules!.top_down = true)}>De haut en bas</button><button class:on={!rules.top_down} onclick={() => (rules!.top_down = false)}>De bas en haut</button></div></div>
            <div class="nb-prev"><span class="flabel">Aperçu</span><b class="mono">{preview(rules, 'V', 2, 1)}</b><span class="mono muted">{preview(rules, 'A', 1, 12)} · {preview(rules, 'S', 3, 7)}</span></div>
          </div>
          <div class="row-actions" style="justify-content:flex-start">
            {#if container && can('projets', 'ecriture')}<button class="btn sm ghost" onclick={() => saveRules('element')}><Icon name="pen" size="sm" />Garder pour {container.name}</button>{/if}
            {#if can('parametres', 'ecriture')}<button class="btn sm ghost" onclick={() => saveRules('site')}><Icon name="pen" size="sm" />Garder pour le site</button>{/if}
          </div>
          <span class="muted small">Préfixe et suffixe viennent du signal (à défaut, de sa fonction) ; sens 1 entrée, 2 sortie, 3 bidirectionnel. Un compteur par préfixe et par sens ; un numéro déjà pris est sauté et n’est jamais recyclé. Les liaisons entre éléments se numérotent au niveau du projet.</span>
        </section>
      {/if}
    </div>
  {/if}
  {#snippet footer()}<button class="btn ghost" onclick={onclose}>Annuler</button><button class="btn primary" class:danger-btn={mode === 'clear'} disabled={busy || !rules} onclick={run}>{#if busy}<span class="spin"></span>{/if}{mode === 'clear' ? 'Effacer' : 'Numéroter'}</button>{/snippet}
</Dialog>

<style>
  .nb-row { display: flex; gap: 14px; flex-wrap: wrap; align-items: flex-end; }
  .nb-rules { display: grid; gap: 12px; padding: 14px; border-radius: 16px; border: 1.5px dashed var(--line); }
  .nb-h { display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; }
  .nb-prev { display: flex; flex-direction: column; gap: 2px; padding: 8px 12px; border-radius: 12px; background: var(--surface-2); }
  .nb-prev b { font-size: 20px; }
  .seg.wrap { flex-wrap: wrap; }
  .ferr { color: var(--err); font-size: 13px; }
  .danger-btn { background: var(--err); color: #fff; }
</style>
