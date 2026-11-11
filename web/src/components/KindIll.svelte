<script lang="ts">
  // Pictogramme d'un type d'élément (galerie « Ajouter un élément », cartes de l'inventaire).
  // Trait à la couleur du texte, pièces actives à la couleur d'accent --ki.
  import type { ContainerKind } from '../lib/projects';
  let { kind, size = 96 }: { kind: ContainerKind; size?: number } = $props();
</script>

<svg class="ki" viewBox="0 0 96 64" width={size} height={(size * 2) / 3} aria-hidden="true">
  {#if kind === 'rack'}
    <rect class="b" x="30" y="4" width="36" height="56" rx="3" />
    {#each [10, 18, 26, 34, 42] as y, i}<rect class={i === 1 ? 'a' : 'f'} x="35" y={y} width="26" height="6" rx="1" />{/each}
    <path d="M33 60v2M63 60v2" />
  {:else if kind === 'flight_case'}
    <rect class="b" x="18" y="10" width="60" height="46" rx="5" />
    <path d="M24 10V6h48v4M18 22h60M18 46h60" />
    {#each [27, 34, 40] as y, i}<rect class={i === 0 ? 'a' : 'f'} x="28" y={y} width="40" height="4.5" rx="1" />{/each}
    <circle class="f" cx="26" cy="58" r="3" /><circle class="f" cx="70" cy="58" r="3" />
  {:else if kind === 'hard_case'}
    <rect class="b" x="22" y="18" width="52" height="38" rx="6" />
    <path d="M40 18v-6h16v6M22 30h52" />
    <rect class="a" x="30" y="37" width="36" height="5" rx="1" /><rect class="f" x="30" y="45" width="36" height="5" rx="1" />
  {:else if kind === 'waterproof_case'}
    <rect class="b" x="16" y="18" width="64" height="38" rx="9" />
    <path d="M38 18v-6h20v6M16 32h64" />
    <rect class="a" x="26" y="29" width="8" height="7" rx="2" /><rect class="a" x="62" y="29" width="8" height="7" rx="2" />
  {:else if kind === 'furniture'}
    <rect class="b" x="24" y="6" width="48" height="52" rx="3" />
    <path d="M24 24h48M24 42h48M48 42v16" />
    <rect class="f" x="30" y="14" width="14" height="8" rx="1" /><rect class="a" x="50" y="30" width="16" height="9" rx="1" />
  {:else if kind === 'monitor_wall'}
    {#each [0, 1] as r}{#each [0, 1, 2] as c}<rect class={r === 0 && c === 1 ? 'a' : 'f'} x={14 + c * 23} y={6 + r * 17} width="21" height="15" rx="1.5" />{/each}{/each}
    <path d="M48 40v14M30 58h36M14 40h68" />
  {:else if kind === 'desk'}
    <path class="b" d="M8 40h80l-6 10H14z" />
    {#each [0, 1, 2] as c}<rect class={c === 1 ? 'a' : 'f'} x={18 + c * 21} y="18" width="18" height="13" rx="1.5" /><path d="M{27 + c * 21} 31v5" />{/each}
    <path d="M18 50v8M78 50v8" /><rect class="f" x="40" y="42" width="16" height="4" rx="1" />
  {:else if kind === 'operator_position'}
    <path class="b" d="M14 42h50l-4 8H18z" />
    <rect class="f" x="26" y="20" width="24" height="16" rx="1.5" /><path d="M38 36v6" />
    <path class="ac" d="M70 30a10 10 0 0 1 20 0" /><rect class="a" x="68" y="29" width="5" height="8" rx="2" /><rect class="a" x="87" y="29" width="5" height="8" rx="2" />
    <path d="M22 50v8M58 50v8" />
  {:else if kind === 'zone'}
    <path class="b dash" d="M10 22l38-14 38 14-38 14z" />
    <path d="M10 22v18l38 14 38-14V22" class="dash" />
    <rect class="a" x="34" y="18" width="12" height="8" rx="1.5" /><rect class="f" x="50" y="22" width="10" height="7" rx="1.5" />
  {:else if kind === 'camera_position'}
    <rect class="b" x="24" y="10" width="34" height="18" rx="3" /><path class="b" d="M58 14l12-5v20l-12-5z" />
    <circle class="a" cx="32" cy="19" r="3" />
    <path d="M41 28v8M41 36l-16 22M41 36l16 22M41 36v22" />
  {:else if kind === 'field_box'}
    <rect class="b" x="18" y="14" width="60" height="36" rx="4" />
    {#each [0, 1, 2, 3] as c}<circle class={c === 0 ? 'a' : 'f'} cx={30 + c * 12} cy="28" r="4" />{/each}
    {#each [0, 1, 2, 3] as c}<circle class="f" cx={30 + c * 12} cy="40" r="3" />{/each}
    <path d="M24 50v6M72 50v6" />
  {:else if kind === 'rf_mast'}
    <path d="M48 16v44M36 60h24M48 30l-14 30M48 30l14 30" />
    <rect class="a" x="44" y="8" width="8" height="10" rx="2" />
    <path class="ac" d="M34 8a16 16 0 0 0 0 14M62 8a16 16 0 0 1 0 14M28 4a24 24 0 0 0 0 22M68 4a24 24 0 0 1 0 22" />
  {:else if kind === 'site_point'}
    <rect class="b" x="22" y="8" width="52" height="48" rx="4" />
    {#each [0, 1] as r}{#each [0, 1, 2] as c}<circle class={r === 0 && c === 0 ? 'a' : 'f'} cx={34 + c * 14} cy={24 + r * 16} r="4.5" />{/each}{/each}
    <circle class="f" cx="27" cy="13" r="1.5" /><circle class="f" cx="69" cy="51" r="1.5" />
  {:else if kind === 'power'}
    <rect class="b" x="12" y="16" width="62" height="36" rx="4" />
    <path d="M12 28h62M74 26h8v18h-8" />
    <path class="a" d="M44 31l-7 10h6l-3 8 9-12h-6l3-6z" />
    <circle class="f" cx="22" cy="56" r="3" /><circle class="f" cx="64" cy="56" r="3" />
  {:else if kind === 'external_feed'}
    <path class="b" d="M18 36a20 20 0 0 1 30-20z" /><path d="M33 26l14-12M47 14l4-4M33 36l-4 22M24 58h18" />
    <path class="ac" d="M56 12a10 10 0 0 1 8 8M58 4a18 18 0 0 1 14 14" />
    <path class="a" d="M54 46h30" /><circle class="a" cx="84" cy="46" r="3" />
  {:else if kind === 'display'}
    <rect class="b" x="10" y="6" width="76" height="40" rx="2" />
    <rect class="a" x="16" y="12" width="64" height="28" rx="1" />
    <path d="M30 46v12M66 46v12M22 58h52" />
  {:else if kind === 'cable_drum'}
    <circle class="b" cx="44" cy="32" r="22" /><circle class="f" cx="44" cy="32" r="7" />
    <path class="ac" d="M44 18a14 14 0 1 1-14 14" />
    <path d="M30 54l-6 6M58 54l6 6M66 32h22" /><circle class="a" cx="88" cy="32" r="3" />
  {/if}
</svg>

<style>
  .ki { display: block; overflow: visible; color: var(--fg-2); --ki: var(--accent, #4F6BFF); }
  .ki :global(*) { fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  .ki .b { fill: color-mix(in srgb, currentColor 7%, transparent); }
  .ki .f { fill: color-mix(in srgb, currentColor 16%, transparent); }
  .ki .a { fill: color-mix(in srgb, var(--ki) 70%, transparent); stroke: var(--ki); }
  .ki .ac { stroke: var(--ki); }
  .ki .dash { stroke-dasharray: 4 3; }
</style>
