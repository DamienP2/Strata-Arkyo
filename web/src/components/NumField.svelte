<script lang="ts">
  // Champ numérique avec unité, valeurs proposées (préréglages) et message d'erreur.
  import FieldError from './FieldError.svelte';
  let { id, label, value = $bindable(null), unit = '', step = 'any', min, max, suggestions = [], error, help, required = false, disabled = false, placeholder, onchange }:
    { id: string; label: string; value: number | null; unit?: string; step?: string | number; min?: number; max?: number; suggestions?: number[]; error?: string | null; help?: string; required?: boolean;
      disabled?: boolean; placeholder?: string;
      /** Appelé à la validation du champ (événement change : sortie du champ ou Entrée), avec la valeur lue. */
      onchange?: (value: number | null) => void } = $props();
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
  // Aide et erreur reliées au champ (lecteurs d'écran).
  const described = $derived(error ? `${id}-err` : help ? `${id}-help` : undefined);
</script>

<div class="field">
  <label for={id} title={help || undefined}>{label}</label>
  <div class="unit-in">
    <input class="input mono" class:err={!!error} {id} inputmode="decimal" value={text} oninput={input} onchange={() => onchange?.(value)} {disabled} {placeholder} list={suggestions.length ? `${id}-list` : undefined} {required} data-step={step} data-min={min} data-max={max} autocomplete="off" aria-invalid={error ? 'true' : undefined} aria-describedby={described}>
    {#if unit}<span class="u">{unit}</span>{/if}
  </div>
  {#if suggestions.length}<datalist id="{id}-list">{#each suggestions as s}<option value={String(s).replace('.', ',')}></option>{/each}</datalist>{/if}
  {#if help && !error}<span class="sr-only" id="{id}-help">{help}</span>{/if}
  <FieldError message={error} for={id} />
</div>
