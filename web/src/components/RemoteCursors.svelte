<script lang="ts">
  // Curseurs et sélections des autres sur une vue graphique (section 12) : une salle par vue,
  // positions lissées, curseur masqué après 5 s d'inactivité, couleur propre à chacun.
  import { onDestroy } from 'svelte';
  import { join, onRemote, live, type PresenceMember } from '../lib/live.svelte';

  let { room, toScreen, selections = $bindable({}) }: {
    room: string;
    toScreen: (x: number, y: number) => [number, number];
    selections?: Record<string, { member: PresenceMember; ids: string[] }>;
  } = $props();

  let cursors = $state<Record<string, { member: PresenceMember; x: number; y: number; at: number }>>({});
  let now = $state(Date.now());
  const tick = setInterval(() => (now = Date.now()), 1000);
  onDestroy(() => clearInterval(tick));
  $effect(() => {
    const r = room;
    const leave = join(r, () => {});
    const off = onRemote(r, (m) => {
      if (m.type === 'cursor' && m.x != null && m.y != null) cursors[m.member.id] = { member: m.member, x: m.x, y: m.y, at: Date.now() };
      if (m.type === 'select') { const s = { ...selections }; if (m.ids?.length) s[m.member.id] = { member: m.member, ids: m.ids }; else delete s[m.member.id]; selections = s; }
    });
    return () => { off(); leave(); cursors = {}; selections = {}; };
  });
  // Quelqu'un quitte la vue : son curseur et sa sélection disparaissent.
  $effect(() => {
    const here = new Set((live.presence[room] ?? []).map((p) => p.id));
    for (const id of Object.keys(cursors)) if (!here.has(id)) delete cursors[id];
    if (Object.keys(selections).some((id) => !here.has(id))) selections = Object.fromEntries(Object.entries(selections).filter(([id]) => here.has(id)));
  });
  const others = $derived((live.presence[room] ?? []));
</script>

<div class="rc" aria-hidden="true">
  {#each Object.values(cursors).filter((c) => now - c.at < 5000) as c (c.member.id)}
    {@const p = toScreen(c.x, c.y)}
    <div class="cur" style="transform:translate({p[0]}px,{p[1]}px);--c:{c.member.color}">
      <svg width="16" height="18" viewBox="0 0 16 18"><path d="M1 1l13 7-6 1.6L5 16z" fill="var(--c)" stroke="#fff" stroke-width="1.2" /></svg>
      <span>{c.member.name}</span>
    </div>
  {/each}
</div>
{#if others.length > 1}
  <div class="rc-who" title="Sur cette vue">{#each others as o (o.id)}<span style="--c:{o.color}">{o.name.split(' ').map((x) => x[0]).join('').slice(0, 2)}</span>{/each}</div>
{/if}

<style>
  .rc { position: absolute; inset: 0; pointer-events: none; overflow: hidden; z-index: 5; }
  .cur { position: absolute; left: 0; top: 0; transition: transform 90ms linear; display: flex; align-items: flex-start; gap: 2px; }
  .cur span { margin-top: 12px; background: var(--c); color: #fff; font: 600 11px var(--font); padding: 1px 6px; border-radius: 6px; white-space: nowrap; }
  .rc-who { position: absolute; right: 10px; top: 10px; display: flex; gap: 4px; z-index: 6; pointer-events: none; }
  .rc-who span { width: 24px; height: 24px; border-radius: 50%; background: var(--c); color: #fff; font: 700 10px var(--font); display: grid; place-items: center; box-shadow: 0 0 0 2px var(--surface); }
</style>
