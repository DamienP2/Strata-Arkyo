<script lang="ts" module>
  /** Article d'un menu contextuel : action, sous-menu (`sub`), séparateur (`sep`) ou intertitre (`heading`). */
  export interface CtxItem {
    label?: string;
    icon?: string;
    /** Précision affichée sous le libellé (à éviter : une ligne courte par action). */
    hint?: string;
    /** Valeur courte alignée à droite, en mono (« 0,5 U »). */
    right?: string;
    danger?: boolean;
    disabled?: boolean;
    /** Infobulle (raison d'un article désactivé). */
    title?: string;
    run?: () => void;
    /** Sous-menu : ouvert à la place du menu, avec un article « Retour ». */
    sub?: CtxItem[];
    sep?: boolean;
    heading?: string;
  }
</script>

<script lang="ts">
  // Menu contextuel (clic droit, touche Menu, Maj+F10) : même apparence et même clavier que les
  // menus déroulants (.pop-menu, menuKeys : flèches, Début/Fin, Échap), rendu dans <body> en
  // position fixe et gardé dans la fenêtre (jamais coupé par une colonne qui défile). Les
  // sous-menus remplacent le menu (flèche droite ou Entrée pour entrer, flèche gauche pour revenir).
  import { tick, untrack } from 'svelte';
  import Icon from './Icon.svelte';
  import { menuKeys } from '../lib/menukeys';
  import { outside } from '../lib/outside';

  let { x, y, items, label, onclose, back }: {
    x: number; y: number; items: CtxItem[];
    /** Nom accessible du menu. */
    label: string;
    onclose: () => void;
    /** Élément qui reprend le focus à la fermeture (par défaut : l'élément actif à l'ouverture). */
    back?: HTMLElement | null;
  } = $props();

  const initial = document.activeElement as HTMLElement | null;
  let stack = $state<{ title: string; items: CtxItem[] }[]>([]);
  const level = $derived(stack.length ? stack[stack.length - 1]! : { title: '', items });
  let node = $state<HTMLDivElement>();

  /** Place le menu au point demandé, retourné vers la gauche ou le haut s'il manque de place. */
  function place() {
    if (!node) return;
    const m = 8, vw = innerWidth, vh = innerHeight;
    node.style.maxHeight = '';
    const w = node.offsetWidth, h = node.scrollHeight;
    const room = vh - 2 * m;
    node.style.maxHeight = h > room ? `${room}px` : '';
    const hh = Math.min(h, room);
    const left = x + w + m > vw ? Math.max(m, x - w) : x;
    const top = y + hh + m > vh ? Math.max(m, vh - hh - m) : y;
    node.style.left = `${left}px`;
    node.style.top = `${top}px`;
  }
  function portal(n: HTMLElement) {
    document.body.appendChild(n);
    place();
    const close = () => onclose();
    addEventListener('resize', close);
    addEventListener('blur', close);
    return { destroy() { removeEventListener('resize', close); removeEventListener('blur', close); n.remove(); } };
  }
  // Élément qui reprend le focus : lu une fois, avant la fermeture (les props ne sont plus lues ensuite).
  const target = untrack(() => back) ?? initial;
  function done() { onclose(); target?.focus?.({ preventScroll: true }); }
  async function refocus() {
    await tick(); place();
    node?.querySelector<HTMLElement>('[role="menuitem"]:not([disabled])')?.focus();
  }
  function enter(it: CtxItem) { if (!it.sub?.length) return; stack = [...stack, { title: it.label ?? '', items: it.sub }]; refocus(); }
  function leave() { if (!stack.length) return; stack = stack.slice(0, -1); refocus(); }
  function pick(it: CtxItem) {
    if (it.disabled) return;
    if (it.sub) { enter(it); return; }
    done();
    it.run?.();
  }
  function key(e: KeyboardEvent) {
    const it = (e.target as HTMLElement).closest<HTMLElement>('[data-sub]');
    if (e.key === 'ArrowRight' && it) { e.preventDefault(); const i = Number(it.dataset.sub); enter(level.items[i]!); }
    else if (e.key === 'ArrowLeft' && stack.length) { e.preventDefault(); leave(); }
  }
  // Sur un écran tactile ou au clavier, le menu prend le focus : menuKeys le pose sur le premier article.
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="pop-menu fixed ctx-menu" role="menu" tabindex="-1" aria-label={stack.length ? level.title : label} bind:this={node}
  use:portal use:outside={() => onclose()} use:menuKeys={{ close: done, trigger: target }} onkeydown={key}
  oncontextmenu={(e) => e.preventDefault()}>
  {#if stack.length}
    <button type="button" role="menuitem" class="ctx-back" onclick={leave}><Icon name="chevr" size="sm" /><span>Retour</span></button>
    <hr>
  {/if}
  {#each level.items as it, i}
    {#if it.sep}<hr>
    {:else if it.heading}<div class="ctx-head" aria-hidden="true">{it.heading}</div>
    {:else}
      <button type="button" role="menuitem" class:danger={it.danger} disabled={it.disabled} title={it.title} aria-haspopup={it.sub ? 'menu' : undefined} data-sub={it.sub ? i : undefined} onclick={() => pick(it)}>
        {#if it.icon}<Icon name={it.icon} size="sm" />{/if}
        <span class="ctx-l">{it.label}{#if it.hint}<span class="pm-sub">{it.hint}</span>{/if}</span>
        {#if it.right}<span class="ctx-r mono">{it.right}</span>{/if}
        {#if it.sub}<span class="ctx-chev"><Icon name="chevr" size="sm" /></span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .ctx-menu { min-width: 240px; max-width: min(340px, calc(100vw - 16px)); z-index: 95; }
  .ctx-l { flex: 1; min-width: 0; }
  .ctx-chev { margin-left: auto; display: inline-flex; }
  .ctx-back :global(.ic) { transform: rotate(180deg); }
  .ctx-r { margin-left: auto; padding-left: 16px; font-size: 11.5px; color: var(--text-3); font-variant-numeric: tabular-nums; white-space: nowrap; }
  button:disabled .ctx-r { color: inherit; }
  .ctx-head { padding: 6px 10px 2px; font: 600 var(--fs-2xs, 11.5px)/1.3 var(--font); letter-spacing: .05em; text-transform: uppercase; color: var(--fg-3); }
</style>
