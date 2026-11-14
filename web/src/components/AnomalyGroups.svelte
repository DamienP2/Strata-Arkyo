<script lang="ts" generics="T">
  // Anomalies regroupées par type : un compteur par type, dépliable, chaque ligne nommant
  // l'objet (câble, port, nœud) et menant à lui (lien, ou action au clic).
  import Icon from './Icon.svelte';
  import { groupAnomalies, type Anomaly, type AnomalyGroup } from '../lib/anomalies';

  let { list, onpick, groupHref, groupLink = 'Tout voir', compact = false }: {
    list: Anomaly<T>[];
    onpick?: (a: Anomaly<T>) => void;
    /** Lien « tout voir » d'un groupe (liste filtrée…). */
    groupHref?: (g: AnomalyGroup<T>) => string | undefined;
    groupLink?: string;
    /** Panneau étroit (synoptique) : détail sous le sujet. */
    compact?: boolean;
  } = $props();
  const groups = $derived(groupAnomalies(list));
  const STEP = 30;
  let shown = $state<Record<string, number>>({});
</script>

<div class="ag" class:compact>
  {#each groups as g (g.key)}
    {@const n = shown[g.key] ?? STEP}
    {@const href = groupHref?.(g)}
    <details class="ag-g {g.severity}" open={groups.length === 1 && g.items.length <= 6}>
      <summary>
        <i class="dot"></i><span class="ag-l">{g.label}</span>
        {#if href}<a class="ag-all small" {href} onclick={(e) => e.stopPropagation()}>{groupLink}</a>{/if}
        <Icon name="chevr" size="sm" />
      </summary>
      <ul>
        {#each g.items.slice(0, n) as a, k (k)}
          <li>
            {#if a.href}
              <a class="ag-it" href={a.href}><span class="ag-t"><b>{a.subject}</b>{#if a.place}<span class="muted">{' · '}{a.place}</span>{/if}</span><span class="ag-d muted small">{a.detail}</span><Icon name="chevr" size="sm" /></a>
            {:else}
              <button class="ag-it" type="button" onclick={() => onpick?.(a)}><span class="ag-t"><b>{a.subject}</b>{#if a.place}<span class="muted">{' · '}{a.place}</span>{/if}</span><span class="ag-d muted small">{a.detail}</span>{#if onpick}<Icon name="chevr" size="sm" />{/if}</button>
            {/if}
          </li>
        {/each}
      </ul>
      {#if g.items.length > n}<button class="btn sm ghost" type="button" onclick={() => (shown[g.key] = n + STEP * 3)}>Afficher {Math.min(STEP * 3, g.items.length - n)} de plus (reste {g.items.length - n})</button>{/if}
    </details>
  {/each}
</div>

<style>
  .ag { display: grid; gap: 6px; }
  .ag-g { border: 1px solid var(--line); border-radius: 12px; background: var(--surface); }
  .ag-g summary { display: flex; gap: 10px; align-items: center; padding: 9px 12px; cursor: pointer; list-style: none; font-weight: 600; font-size: 14px; }
  .ag-g summary::-webkit-details-marker { display: none; }
  .ag-g summary :global(svg:last-child) { transition: transform .15s; color: var(--fg-3); }
  .ag-g[open] summary :global(svg:last-child) { transform: rotate(90deg); }
  .ag-l { flex: 1; }
  .ag-all { font-weight: 500; color: var(--accent, #4F6BFF); text-decoration: none; }
  .ag-all:hover { text-decoration: underline; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--fg-3); flex: none; }
  .error .dot { background: var(--err); }
  .warning .dot { background: var(--warn, #B78400); }
  ul { list-style: none; margin: 0; padding: 0 6px 6px; display: grid; }
  .ag-it { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr) auto; gap: 10px; align-items: baseline; width: 100%; padding: 6px 8px; border: 0; border-radius: 8px; background: none; color: inherit; text-decoration: none; text-align: left; font: inherit; font-size: 14px; cursor: pointer; }
  .ag-it:hover { background: var(--surface-2); }
  .ag-t { min-width: 0; overflow-wrap: anywhere; }
  .ag-it :global(svg) { align-self: center; color: var(--fg-3); }
  .compact .ag-it { grid-template-columns: minmax(0, 1fr) auto; row-gap: 2px; align-items: start; }
  .compact .ag-d { grid-column: 1; }
  .compact .ag-it :global(svg) { grid-row: 1 / span 2; grid-column: 2; }
  .ag-g > .btn { margin: 0 0 8px 14px; }
  @media (max-width: 720px) { .ag-it { grid-template-columns: minmax(0, 1fr) auto; } .ag-d { grid-column: 1; } .ag-it :global(svg) { grid-row: 1 / span 2; grid-column: 2; } }
</style>
