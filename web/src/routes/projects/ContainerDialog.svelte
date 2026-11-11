<script lang="ts">
  import { untrack } from 'svelte';
  import Dialog from '../../components/Dialog.svelte';
  import NumField from '../../components/NumField.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import { api, ApiError } from '../../lib/api';
  import { toast } from '../../lib/toast.svelte';
  import { CONTAINER_KINDS, U_PRESETS, blankContainer, type Container, type ContainerKind, type ProjectView } from '../../lib/projects';

  let { view, container = null, locationId, onclose, ondone }: { view: ProjectView; container?: Container | null; locationId: string; onclose: () => void; ondone: (id: string) => void } = $props();
  // Le dialogue part d'une copie : les changements ne s'appliquent qu'à l'enregistrement.
  let name = $state(untrack(() => container?.name ?? `Contenant ${view.containers.length + 1}`));
  let loc = $state(untrack(() => container?.location_id ?? locationId));
  let c = $state(untrack(() => (container ? { ...container } : blankContainer('rack'))));
  let errors = $state<Record<string, string>>({});
  let general = $state<string | null>(null);

  function setKind(k: ContainerKind) {
    if (container) { c.kind = k; if (!CONTAINER_KINDS[k].mobile) c.wheels = false; return; }
    c = { ...blankContainer(k), empty_weight_kg: c.empty_weight_kg, max_weight_kg: c.max_weight_kg, max_power_w: c.max_power_w };
  }
  async function save(e: SubmitEvent) {
    e.preventDefault(); errors = {}; general = null;
    const body = { kind: c.kind, rackable: c.rackable, height_u: c.height_u, depth_mm: c.depth_mm, rear_rails_mm: c.rear_rails_mm, length_mm: c.length_mm, width_mm: c.width_mm, outer_height_mm: c.outer_height_mm, wheels: c.wheels, empty_weight_kg: c.empty_weight_kg, max_weight_kg: c.max_weight_kg, max_power_w: c.max_power_w, cable_code: c.cable_code, notes: c.notes, name, location_id: loc };
    try {
      const r = container ? await api.patch<Container>(`/containers/${container.id}`, { ...body, rev: container.rev }) : await api.post<Container>(`/projects/${view.project.id}/containers`, body);
      toast(container ? 'Contenant enregistré.' : r.name !== name.trim() ? `Contenant ajouté sous le nom « ${r.name} ».` : 'Contenant ajouté.');
      ondone(r.id);
    } catch (x) {
      if (x instanceof ApiError) { errors = Object.fromEntries(x.fields.map((f) => [f.field, f.message])); general = x.fields.length ? null : x.message; } else general = 'Enregistrement impossible.';
    }
  }
</script>

<Dialog title={container ? `Modifier ${container.name}` : 'Nouveau contenant'} eyebrow={view.project.name} {onclose} wide>
  <form id="contf" class="stack" onsubmit={save}>
    <div class="choices" style="grid-template-columns:repeat(auto-fit,minmax(130px,1fr))">
      {#each Object.entries(CONTAINER_KINDS) as [k, v]}<button type="button" class="choice" class:on={c.kind === k} onclick={() => setKind(k as ContainerKind)}><b style="font-size:17px">{v.label}</b></button>{/each}
    </div>
    <div class="fg3">
      <div class="field"><label for="c-name">Nom</label><input class="input" class:err={!!errors.name} id="c-name" bind:value={name} required maxlength="120"><FieldError message={errors.name} /></div>
      <div class="field"><label for="c-loc">Emplacement</label><select class="input" id="c-loc" bind:value={loc}>{#each view.locations as l}<option value={l.id}>{l.name}</option>{/each}</select><FieldError message={errors.location_id} /></div>
      <div class="field"><label for="c-code">Code câble</label><input class="input mono" id="c-code" bind:value={c.cable_code} maxlength="5" placeholder="Tiré du nom"><span class="help">5 caractères : majuscules et chiffres.</span></div>
    </div>
    <div style="display:flex;gap:18px;flex-wrap:wrap">
      <label class="toggle-row"><input type="checkbox" bind:checked={c.rackable}>Rackable (19″)</label>
      {#if CONTAINER_KINDS[c.kind].mobile}<label class="toggle-row"><input type="checkbox" bind:checked={c.wheels}>Roulettes</label>{/if}
    </div>
    <div class="fg4">
      {#if c.rackable}
        <NumField id="c-u" label="Hauteur" unit="U" bind:value={c.height_u} suggestions={U_PRESETS} error={errors.height_u} />
        <NumField id="c-d" label="Profondeur" unit="mm" bind:value={c.depth_mm} error={errors.depth_mm} />
        <NumField id="c-r" label="Rails arrière" unit="mm" bind:value={c.rear_rails_mm} error={errors.rear_rails_mm} help="Distance au fond ; vide sans rails." />
      {:else}
        <NumField id="c-l" label="Longueur" unit="mm" bind:value={c.length_mm} error={errors.length_mm} />
        <NumField id="c-w" label="Largeur" unit="mm" bind:value={c.width_mm} error={errors.width_mm} />
        <NumField id="c-d" label="Profondeur" unit="mm" bind:value={c.depth_mm} error={errors.depth_mm} />
      {/if}
      <NumField id="c-kg" label="Poids à vide" unit="kg" bind:value={c.empty_weight_kg} error={errors.empty_weight_kg} />
      <NumField id="c-mkg" label="Poids maximum" unit="kg" bind:value={c.max_weight_kg} error={errors.max_weight_kg} />
      <NumField id="c-mw" label="Puissance maximum" unit="W" bind:value={c.max_power_w} error={errors.max_power_w} />
    </div>
    <div class="field"><label for="c-notes">Notes</label><textarea class="input" id="c-notes" rows="2" style="height:auto;padding:12px 14px" value={c.notes ?? ''} oninput={(e) => (c.notes = (e.target as HTMLTextAreaElement).value || null)}></textarea></div>
    {#if general}<div class="alert err">{general}</div>{/if}
  </form>
  {#snippet footer()}<button class="btn ghost" onclick={onclose}>Annuler</button><button class="btn primary" type="submit" form="contf">{container ? 'Enregistrer' : 'Ajouter le contenant'}</button>{/snippet}
</Dialog>
