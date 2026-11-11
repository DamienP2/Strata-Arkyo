<script lang="ts">
  // Champ numérique avec unité, valeurs proposées (préréglages) et message d'erreur.
  import FieldError from './FieldError.svelte';
  let { id, label, value = $bindable(null), unit = '', step = 'any', min, max, suggestions = [], error, help, required = false }:
    { id: string; label: string; value: number | null; unit?: string; step?: string | number; min?: number; max?: number; suggestions?: number[]; error?: string; help?: string; required?: boolean } = $props();
  let text = $state(value == null ? '' : String(value).replace('.', ','));
  $effect(() => {
    const cur = text.trim() === '' ? null : Number(text.replace(',', '.').replace(/\s/g, ''));
    if (cur !== value && !(Number.isNaN(cur))) text = value == null ? '' : String(value).replace('.', ',');
  });
  function input(e: Event) {
    text = (e.target as HTMLInputElement).value;
    const t = text.trim().replace(',', '.').replace(/\s/g, '');
    if (t === '') value = null;
    else if (!Number.isNaN(Number(t))) value = Number(t);
  }
</script>

<div class="field">
  <label for={id}>{label}</label>
  <div class="unit-in">
    <input class="input mono" class:err={!!error} {id} inputmode="decimal" value={text} oninput={input} list={suggestions.length ? `${id}-list` : undefined} {required} data-step={step} data-min={min} data-max={max} autocomplete="off">
    {#if unit}<span class="u">{unit}</span>{/if}
  </div>
  {#if suggestions.length}<datalist id="{id}-list">{#each suggestions as s}<option value={String(s).replace('.', ',')}></option>{/each}</datalist>{/if}
  {#if help && !error}<span class="help">{help}</span>{/if}
  <FieldError message={error} />
</div>
