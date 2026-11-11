<script lang="ts">
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import ChipPicker from '../../components/ChipPicker.svelte';
  import NumField from '../../components/NumField.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { REF_KINDS, refs, refList, refName, loadRefs, type RefItem, type RefPath } from '../../lib/catalog.svelte';

  const kind = $derived((REF_KINDS.find((k) => k.path === route.path.split('/')[3])?.path ?? 'functions') as RefPath);
  const meta = $derived(REF_KINDS.find((k) => k.path === kind)!);
  const writable = $derived(can('parametres', 'ecriture'));
  const list = $derived(refs.byKind[kind] ?? []);

  let editing = $state<{ id: string | null; rev: number; name: string; parent_id: string | null; data: any } | null>(null);
  let errors = $state<Record<string, string>>({});
  let general = $state<string | null>(null);
  let deleting = $state<RefItem | null>(null);

  const DIM_KINDS: [string, string, string][] = [['height_u', 'Hauteur', 'U'], ['width', 'Largeur (fraction)', ''], ['depth_mm', 'Profondeur', 'mm'], ['rear_reserve_mm', 'Réserve arrière', 'mm'], ['cable_length_m', 'Longueur de câble', 'm'], ['bend_radius_mm', 'Rayon de courbure', 'mm']];
  const REDUNDANCY: [string, string][] = [['', 'Aucune'], ['ab', 'A / B'], ['primary_secondary', 'Principal / secours'], ['blue_red', 'Bleu / rouge']];

  function blankData(k: RefPath): any {
    switch (k) {
      case 'functions': return { key: '', color: '#5E626A', cable_prefix: '' };
      case 'signals': return { color: '#2459E0', function: 'V', prefix: '', suffix: '', impedance_ohm: null, min_bandwidth_mhz: null, connectors: [], ip: false, multicast: false, redundancy: null, default: false };
      case 'connectors': return { variants: [], symbol: '', color: '#5E626A', functions: [] };
      case 'brands': return { website: null };
      case 'categories': return { color: '', icon: '' };
      case 'types': return { function: 'V', icon: '' };
      case 'dimensions': return { kind: 'height_u', value: null, reel: false };
      default: return {};
    }
  }
  function edit(r?: RefItem) {
    errors = {}; general = null;
    editing = r ? { id: r.id, rev: r.rev, name: r.name, parent_id: r.parent_id, data: structuredClone(r.data) } : { id: null, rev: 0, name: '', parent_id: null, data: blankData(kind) };
    variants = (editing.data.variants ?? []).join(', ');
  }
  let variants = $state('');
  $effect(() => { if (editing && kind === 'connectors') editing.data.variants = variants.split(',').map((v) => v.trim()).filter(Boolean); });

  async function save(e: SubmitEvent) {
    e.preventDefault();
    errors = {}; general = null;
    const b = editing!;
    try {
      if (b.id) await api.patch(`/catalog/refs/${kind}/${encodeURIComponent(b.id)}`, { rev: b.rev, name: b.name, parent_id: b.parent_id, data: b.data });
      else await api.post(`/catalog/refs/${kind}`, { name: b.name, parent_id: b.parent_id, data: b.data });
      toast(b.id ? 'Modification enregistrée.' : `${meta.one} ajouté(e).`);
      editing = null; await loadRefs(true);
    } catch (x) {
      if (x instanceof ApiError) { errors = Object.fromEntries(x.fields.map((f) => [f.field, f.message])); general = x.message; } else fail(x);
    }
  }
  async function setActive(r: RefItem, active: boolean) {
    try { await api.patch(`/catalog/refs/${kind}/${encodeURIComponent(r.id)}`, { rev: r.rev, active }); await loadRefs(true); } catch (e) { fail(e); }
  }
  async function remove() {
    try { await api.del(`/catalog/refs/${kind}/${encodeURIComponent(deleting!.id)}`); toast('Supprimé.'); deleting = null; await loadRefs(true); } catch (e) { fail(e); deleting = null; }
  }
  async function restoreDims() {
    try { const r = await api.post<{ restored: number }>('/catalog/refs/dimensions/restore'); toast(r.restored ? `${r.restored} préréglage(s) remis.` : 'Tous les préréglages d’origine sont déjà là.'); await loadRefs(true); } catch (e) { fail(e); }
  }
  const sw = (c: string) => (c ? `<span class="role-dot" style="--c:${c}"></span>` : '');
  function detail(r: RefItem): string {
    const d = r.data;
    switch (kind) {
      case 'functions': return `Clé ${d.key} · préfixe ${d.cable_prefix || '—'}`;
      case 'signals': return [refName('functions', d.function), (d.connectors ?? []).map((c: string) => refName('connectors', c)).join(', '), d.impedance_ohm ? `${d.impedance_ohm} Ω` : '', d.ip ? 'IP' + (d.multicast ? ' multicast' : '') : ''].filter(Boolean).join(' · ');
      case 'connectors': return [(d.variants ?? []).join(', '), (d.functions ?? []).map((f: string) => refName('functions', f)).join(', ')].filter(Boolean).join(' · ');
      case 'brands': return d.website ?? '';
      case 'categories': return r.parent_id ? `dans ${refName('categories', r.parent_id)}` : '';
      case 'types': return r.parent_id ? `sous-type de ${refName('types', r.parent_id)}` : `fonction ${refName('functions', d.function)}`;
      case 'system-categories': return `fonction ${refName('functions', d.function)}`;
      case 'dimensions': return DIM_KINDS.find((k) => k[0] === d.kind)?.[1] + (d.reel ? ' · touret' : '');
      default: return '';
    }
  }
</script>

<div class="seg" role="tablist" aria-label="Référentiels">
  {#each REF_KINDS as k}<button role="tab" class:on={k.path === kind} aria-selected={k.path === kind} onclick={() => navigate(`/catalogue/referentiels/${k.path}`)}>{k.label}</button>{/each}
</div>

<div class="page-head" style="align-items:center">
  <p class="muted" style="max-width:62ch">{meta.help}{#if !writable} Modifier les référentiels demande le droit « Paramètres et site : écriture ».{/if}</p>
  {#if writable && kind !== 'system-categories'}
    <div class="head-actions">
      {#if kind === 'dimensions'}<button class="btn" onclick={restoreDims}><Icon name="undo" />Remettre ceux d’origine</button>{/if}
      <button class="btn primary" onclick={() => edit()}><Icon name="plus" />Ajouter</button>
    </div>
  {/if}
</div>

<section class="card" style="padding:6px 18px 10px">
  <div class="tbl-wrap">
    <table class="tbl">
      <thead><tr><th>Nom</th><th>Détail</th><th class="num">Utilisé par</th><th>État</th><th></th></tr></thead>
      <tbody>
        {#each list as r (r.id)}
          <tr class:muted={!r.active}>
            <td><div style="display:flex;gap:10px;align-items:center">{@html sw(r.data?.color ?? '')}<b>{r.name}</b>{#if r.protected}<Icon name="lock" size="sm" />{/if}</div></td>
            <td class="small">{detail(r)}</td>
            <td class="num">{r.uses || '—'}</td>
            <td>{#if writable && !r.protected}<label class="toggle-row"><input type="checkbox" checked={r.active} onchange={(e) => setActive(r, (e.target as HTMLInputElement).checked)}>{r.active ? 'Actif' : 'Inactif'}</label>{:else}{r.active ? 'Actif' : 'Inactif'}{/if}</td>
            <td><div class="dl-actions">{#if writable && !r.protected}
              <button class="btn sm ghost" title="Modifier" onclick={() => edit(r)}><Icon name="pen" size="sm" /></button>
              <button class="btn sm ghost danger" title={r.uses ? 'Utilisé : désactivez-le plutôt' : 'Supprimer'} disabled={r.uses > 0} onclick={() => (deleting = r)}><Icon name="trash" size="sm" /></button>
            {/if}</div></td>
          </tr>
        {:else}<tr><td colspan="5" class="empty">Aucun élément.</td></tr>{/each}
      </tbody>
    </table>
  </div>
</section>

{#if editing}
  <Dialog title={editing.id ? `Modifier « ${editing.name} »` : `Ajouter : ${meta.one.toLowerCase()}`} eyebrow={meta.label} onclose={() => (editing = null)}>
    <form id="reff" class="stack" onsubmit={save}>
      {#if kind !== 'dimensions'}
        <div class="field"><label for="rf-name">Nom</label><input class="input" class:err={!!errors.name} id="rf-name" bind:value={editing.name} required maxlength="100"><FieldError message={errors.name} /></div>
      {/if}
      {#if kind === 'functions'}
        <div class="fg3">
          <div class="field"><label for="rf-key">Clé</label><input class="input mono" id="rf-key" bind:value={editing.data.key} disabled={!!editing.id} maxlength="16" required><FieldError message={errors.key} /></div>
          <div class="field"><label for="rf-pre">Préfixe des câbles</label><input class="input mono" id="rf-pre" bind:value={editing.data.cable_prefix} maxlength="4"><FieldError message={errors.cable_prefix} /></div>
          <div class="field"><label for="rf-col">Couleur</label><input class="input" type="color" id="rf-col" bind:value={editing.data.color} style="padding:4px"></div>
        </div>
      {:else if kind === 'signals'}
        <div class="fg3">
          <div class="field"><label for="rf-fn">Fonction</label><select class="input" id="rf-fn" bind:value={editing.data.function}>{#each refList('functions') as f}<option value={f.id}>{f.name}</option>{/each}</select><FieldError message={errors.function} /></div>
          <div class="field"><label for="rf-pre">Préfixe</label><input class="input mono" id="rf-pre" bind:value={editing.data.prefix} maxlength="4"><FieldError message={errors.prefix} /></div>
          <div class="field"><label for="rf-col">Couleur</label><input class="input" type="color" id="rf-col" bind:value={editing.data.color} style="padding:4px"></div>
          <NumField id="rf-imp" label="Impédance" unit="Ω" bind:value={editing.data.impedance_ohm} error={errors.impedance_ohm} />
          <NumField id="rf-bw" label="Bande passante minimale" unit="MHz" bind:value={editing.data.min_bandwidth_mhz} error={errors.min_bandwidth_mhz} />
          <div class="field"><label for="rf-red">Redondance</label><select class="input" id="rf-red" value={editing.data.redundancy ?? ''} onchange={(e) => (editing!.data.redundancy = (e.target as HTMLSelectElement).value || null)}>{#each REDUNDANCY as [k, l]}<option value={k}>{l}</option>{/each}</select></div>
        </div>
        <div class="field"><span class="flabel">Connecteurs compatibles</span><ChipPicker options={refList('connectors').map((c) => ({ id: c.id, name: c.name }))} bind:value={editing.data.connectors} label="Connecteurs" /><FieldError message={errors.connectors} /></div>
        <div style="display:flex;gap:18px;flex-wrap:wrap"><label class="toggle-row"><input type="checkbox" bind:checked={editing.data.ip}>Signal IP</label><label class="toggle-row"><input type="checkbox" bind:checked={editing.data.multicast} disabled={!editing.data.ip}>Multicast</label><label class="toggle-row"><input type="checkbox" bind:checked={editing.data.default}>Proposé par défaut pour ses connecteurs</label></div>
        <FieldError message={errors.multicast} />
      {:else if kind === 'connectors'}
        <div class="form-grid">
          <div class="field"><label for="rf-var">Variantes</label><input class="input" id="rf-var" bind:value={variants} placeholder="ex. 75 Ω, 50 Ω"><FieldError message={errors.variants} /></div>
          <div class="field"><label for="rf-col">Couleur</label><input class="input" type="color" id="rf-col" bind:value={editing.data.color} style="padding:4px"></div>
        </div>
        <div class="field"><span class="flabel">Fonctions</span><ChipPicker options={refList('functions').map((f) => ({ id: f.id, name: f.name, color: f.data.color }))} bind:value={editing.data.functions} label="Fonctions" /></div>
      {:else if kind === 'brands'}
        <div class="field"><label for="rf-web">Site web</label><input class="input mono" id="rf-web" value={editing.data.website ?? ''} oninput={(e) => (editing!.data.website = (e.target as HTMLInputElement).value || null)} placeholder="https://…"><FieldError message={errors.website} /></div>
      {:else if kind === 'categories'}
        <div class="form-grid">
          <div class="field"><label for="rf-par">Dans la catégorie</label><select class="input" id="rf-par" value={editing.parent_id ?? ''} onchange={(e) => (editing!.parent_id = (e.target as HTMLSelectElement).value || null)}><option value="">— (premier niveau)</option>{#each refList('categories').filter((c) => c.id !== editing!.id) as c}<option value={c.id}>{c.name}</option>{/each}</select><FieldError message={errors.parent_id} /></div>
          <div class="field"><label for="rf-col">Couleur</label><input class="input" type="color" id="rf-col" value={editing.data.color || '#5E626A'} oninput={(e) => (editing!.data.color = (e.target as HTMLInputElement).value)} style="padding:4px"></div>
        </div>
      {:else if kind === 'types'}
        <div class="form-grid">
          <div class="field"><label for="rf-par">Sous-type de</label><select class="input" id="rf-par" value={editing.parent_id ?? ''} onchange={(e) => (editing!.parent_id = (e.target as HTMLSelectElement).value || null)}><option value="">— (type principal)</option>{#each refList('types').filter((t) => !t.parent_id && t.id !== editing!.id) as t}<option value={t.id}>{t.name}</option>{/each}</select><FieldError message={errors.parent_id} /></div>
          {#if !editing.parent_id}<div class="field"><label for="rf-fn">Fonction</label><select class="input" id="rf-fn" bind:value={editing.data.function}>{#each refList('functions') as f}<option value={f.id}>{f.name}</option>{/each}</select><FieldError message={errors.function} /></div>{:else}<p class="muted small" style="align-self:end">Un sous-type reprend la fonction de son type.</p>{/if}
        </div>
      {:else if kind === 'dimensions'}
        <div class="form-grid">
          <div class="field"><label for="rf-dk">Genre</label><select class="input" id="rf-dk" bind:value={editing.data.kind}>{#each DIM_KINDS as [k, l]}<option value={k}>{l}</option>{/each}</select></div>
          <NumField id="rf-dv" label="Valeur" unit={DIM_KINDS.find((d) => d[0] === editing!.data.kind)?.[2] ?? ''} bind:value={editing.data.value} error={errors.value} required />
          {#if editing.data.kind === 'cable_length_m'}<label class="toggle-row span"><input type="checkbox" bind:checked={editing.data.reel}>Livré sur touret</label>{/if}
        </div>
      {/if}
      {#if general && !Object.keys(errors).length}<div class="alert err" role="alert">{general}</div>{/if}
    </form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (editing = null)}>Annuler</button><button class="btn primary" type="submit" form="reff">Enregistrer</button>{/snippet}
  </Dialog>
{/if}
{#if deleting}
  <Confirm title="Supprimer « {deleting.name} » ?" eyebrow={meta.label} confirmLabel="Supprimer" danger onconfirm={remove} onclose={() => (deleting = null)}>
    <p>Aucune fiche ne l’utilise. La suppression est définitive ; pour le masquer seulement, désactivez-le.</p>
  </Confirm>
{/if}
