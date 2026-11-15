<script lang="ts" module>
  /** Option d'une commande segmentée. `count` s'affiche après le libellé (`Libellé n`). */
  export interface SegOption<V> { value: V; label: string; icon?: string; title?: string; disabled?: boolean; count?: number | string }
</script>

<script lang="ts" generics="T extends string | number | boolean | null">
  // Commande segmentée .seg : onglets (role=tablist, aria-selected) ou choix exclusif (aria-pressed), .on sur l'actif, flèches/Début/Fin au clavier.
  import Icon from './Icon.svelte';
  let { options, value = $bindable(), mode = 'choice', label, full = false, iconOnly = false, controls, onchange, class: cls = '' }: {
    options: SegOption<T>[];
    value: T;
    /** `tabs` : change de panneau (role=tab, aria-selected, tabindex itinérant) ; `choice` : choix ou filtre (aria-pressed). */
    mode?: 'tabs' | 'choice';
    /** Nom accessible du groupe (« Face », « Vue »…). */
    label: string;
    /** Boutons en flex:1 (panneaux latéraux). */
    full?: boolean;
    /** Icônes seules : le libellé passe en aria-label et title. */
    iconOnly?: boolean;
    /** Mode onglets : préfixe d'id des panneaux (aria-controls = `${controls}-${value}`). */
    controls?: string;
    onchange?: (v: T) => void;
    class?: string;
  } = $props();
  let root = $state<HTMLDivElement>();
  function pick(v: T) { if (v === value) return; value = v; onchange?.(v); }
  function key(e: KeyboardEvent) {
    const keys = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'];
    if (!keys.includes(e.key)) return;
    const enabled = options.filter((o) => !o.disabled);
    if (!enabled.length) return;
    e.preventDefault();
    const i = enabled.findIndex((o) => o.value === value);
    const n = e.key === 'Home' ? 0 : e.key === 'End' ? enabled.length - 1
      : (i + (e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 1) + enabled.length) % enabled.length;
    const o = enabled[n]!;
    pick(o.value);
    root?.querySelectorAll<HTMLButtonElement>('button')[options.indexOf(o)]?.focus();
  }
</script>

<div bind:this={root} class="seg {cls}" class:full role={mode === 'tabs' ? 'tablist' : 'group'} aria-label={label} tabindex="-1" onkeydown={key}>
  {#each options as o (o.value)}
    {@const on = o.value === value}
    <button type="button" class:on disabled={o.disabled}
      role={mode === 'tabs' ? 'tab' : undefined}
      aria-selected={mode === 'tabs' ? on : undefined}
      aria-pressed={mode === 'choice' ? on : undefined}
      aria-controls={mode === 'tabs' && controls ? `${controls}-${o.value}` : undefined}
      tabindex={mode === 'tabs' ? (on ? 0 : -1) : undefined}
      aria-label={iconOnly ? o.label : undefined} title={o.title ?? (iconOnly ? o.label : undefined)}
      onclick={() => pick(o.value)}>
      {#if o.icon}<Icon name={o.icon} size="sm" />{/if}{#if !iconOnly}{o.label}{/if}{#if o.count != null}<span class="muted mono">{o.count}</span>{/if}
    </button>
  {/each}
</div>
