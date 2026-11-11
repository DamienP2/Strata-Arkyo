<script lang="ts">
  // Mur d'écrans : grille de tuiles ; chaque tuile porte un écran du matériel de l'élément et
  // la source qu'il affiche. Cliquer une case vide crée une tuile ; on agrandit une tuile en
  // largeur ou en hauteur pour un écran qui couvre plusieurs cases.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import type { Wall, WallTile } from '../../lib/projects';

  interface Screen { id: string; tag: string; label: string }
  let { wall, screens, writable, onsave }: { wall: Wall; screens: Screen[]; writable: boolean; onsave: (w: Wall) => Promise<boolean> } = $props();
  let w = $state<Wall>(untrack(() => JSON.parse(JSON.stringify(wall))));
  let base = $state(untrack(() => JSON.stringify(wall)));
  $effect(() => { const s = JSON.stringify(wall); if (s !== untrack(() => base)) { w = JSON.parse(s); base = s; sel = null; } });
  const dirty = $derived(JSON.stringify(w) !== base);
  let sel = $state<WallTile | null>(null);
  let saving = $state(false);
  const SOURCES = ['PGM', 'PVW', 'Multiviewer 1', 'Multiviewer 2', 'Clean feed', 'Retour antenne', 'Ralentis', 'Graphisme', ...Array.from({ length: 12 }, (_, i) => `CAM ${i + 1}`)];

  const at = (r: number, c: number) => w.tiles.find((t) => r >= t.row && r < t.row + t.h && c >= t.col && c < t.col + t.w);
  const free = (t: WallTile, nw: number, nh: number) => t.col + nw - 1 <= w.cols && t.row + nh - 1 <= w.rows
    && w.tiles.every((o) => o === t || o.row >= t.row + nh || o.row + o.h <= t.row || o.col >= t.col + nw || o.col + o.w <= t.col);
  function add(r: number, c: number) {
    if (!writable) return;
    const t: WallTile = { row: r, col: c, w: 1, h: 1, instance_id: null, source: '', label: '' };
    w.tiles.push(t);
    sel = w.tiles[w.tiles.length - 1]!;
  }
  function grow(t: WallTile, dw: number, dh: number) {
    const nw = Math.max(1, t.w + dw), nh = Math.max(1, t.h + dh);
    if (free(t, nw, nh)) { t.w = nw; t.h = nh; }
  }
  const used = $derived(new Set(w.tiles.map((t) => t.instance_id).filter(Boolean)));
  const screenOf = (id: string | null) => screens.find((s) => s.id === id);
  async function save() { saving = true; try { if (await onsave($state.snapshot(w) as Wall)) base = JSON.stringify(w); } finally { saving = false; } }
  const cells = $derived(Array.from({ length: w.rows * w.cols }, (_, i) => ({ r: Math.floor(i / w.cols) + 1, c: (i % w.cols) + 1 })));
  const placed = $derived(screens.filter((s) => used.has(s.id)).length);
</script>

<div class="wv">
  <div class="wv-wall" style="--r:{w.rows};--c:{w.cols}">
    {#each cells as x (`${x.r}-${x.c}`)}
      {#if !at(x.r, x.c)}<button class="wv-empty" style="grid-row:{x.r};grid-column:{x.c}" disabled={!writable} onclick={() => add(x.r, x.c)} aria-label="Créer une tuile rangée {x.r}, colonne {x.c}">{#if writable}<Icon name="plus" size="sm" />{/if}</button>{/if}
    {/each}
    {#each w.tiles as t, i (i)}
      {@const s = screenOf(t.instance_id)}
      <button class="wv-tile" class:on={sel === t} class:noscreen={!s} style="grid-row:{t.row} / span {t.h};grid-column:{t.col} / span {t.w}" onclick={() => (sel = t)}>
        <span class="wv-src">{t.source || 'Source ?'}</span>
        <span class="wv-meta mono">{s ? s.tag : 'sans écran'}{t.label ? ` · ${t.label}` : ''}</span>
        <i class="wv-led"></i>
      </button>
    {/each}
  </div>
  <div class="wv-base"></div>
  <p class="muted small" style="margin:8px 0 0">{w.tiles.length} tuile{w.tiles.length > 1 ? 's' : ''} · {placed} / {screens.length} écran{screens.length > 1 ? 's' : ''} placé{placed > 1 ? 's' : ''}{w.support ? ` · ${w.support}` : ''}{writable ? ' · cliquez une case vide pour créer une tuile' : ''}</p>

  {#if sel && writable}
    {@const t = sel}
    <div class="wv-form">
      <span class="lbl-t">Tuile · rangée {t.row}, colonne {t.col}</span>
      <div class="fg3">
        <div class="field"><label for="wv-s">Écran</label><select class="input" id="wv-s" bind:value={t.instance_id}><option value={null}>Aucun</option>{#each screens as s}<option value={s.id} disabled={used.has(s.id) && s.id !== t.instance_id}>{s.tag} · {s.label}</option>{/each}</select></div>
        <div class="field"><label for="wv-src">Source affichée</label><input class="input" id="wv-src" list="wv-sources" bind:value={t.source} maxlength="60" placeholder="PGM, CAM 3…"><datalist id="wv-sources">{#each SOURCES as v}<option value={v}></option>{/each}</datalist></div>
        <div class="field"><label for="wv-l">Libellé</label><input class="input" id="wv-l" bind:value={t.label} maxlength="60" placeholder="Réalisateur, son…"></div>
      </div>
      <div class="wv-acts">
        <span class="muted small">Taille {t.w} × {t.h}</span>
        <button class="btn sm" onclick={() => grow(t, 1, 0)} disabled={!free(t, t.w + 1, t.h)}>Élargir</button>
        <button class="btn sm" onclick={() => grow(t, -1, 0)} disabled={t.w === 1}>Rétrécir</button>
        <button class="btn sm" onclick={() => grow(t, 0, 1)} disabled={!free(t, t.w, t.h + 1)}>Agrandir en hauteur</button>
        <button class="btn sm" onclick={() => grow(t, 0, -1)} disabled={t.h === 1}>Réduire en hauteur</button>
        <span style="flex:1"></span>
        <button class="btn sm ghost danger" onclick={() => { w.tiles = w.tiles.filter((x) => x !== t); sel = null; }}><Icon name="trash" size="sm" />Retirer la tuile</button>
      </div>
    </div>
  {/if}
  {#if writable}
    <div class="row-actions" style="margin-top:10px">
      {#if !screens.length}<span class="muted small" style="margin-right:auto">Ajoutez des écrans à l’élément (« Ajouter du matériel ») pour les placer sur le mur.</span>{/if}
      <button class="btn sm ghost" disabled={!dirty || saving} onclick={() => { w = JSON.parse(base); sel = null; }}>Annuler</button>
      <button class="btn sm primary" disabled={!dirty || saving} onclick={save}>{#if saving}<span class="spin"></span>{/if}Enregistrer le mur</button>
    </div>
  {/if}
</div>

<style>
  .wv { margin-top: 14px; }
  .wv-wall { display: grid; grid-template-columns: repeat(var(--c), minmax(0, 1fr)); grid-template-rows: repeat(var(--r), auto); gap: 6px; padding: 12px; border-radius: 14px; background: radial-gradient(120% 140% at 50% 0%, #2B3038, #121417 70%); max-width: 980px; }
  .wv-empty, .wv-tile { aspect-ratio: 16 / 9; border-radius: 6px; font: inherit; cursor: pointer; min-width: 0; }
  .wv-empty { border: 1.5px dashed #3E454E; background: transparent; color: #6F7882; display: grid; place-items: center; }
  .wv-empty:hover:not(:disabled) { border-color: #9AA3AD; color: #fff; }
  .wv-empty:disabled { cursor: default; }
  .wv-tile { aspect-ratio: auto; position: relative; display: flex; flex-direction: column; justify-content: flex-end; align-items: flex-start; gap: 2px; padding: 10px; border: 3px solid #0B0C0E; color: #fff; text-align: left;
    background: linear-gradient(135deg, #24406E, #142236 60%, #0F1826); box-shadow: 0 0 0 1px #3A4048, 0 6px 14px -6px rgba(0, 0, 0, .6); min-height: 54px; }
  .wv-tile.noscreen { background: repeating-linear-gradient(135deg, #1B1F24 0 8px, #22272D 8px 16px); }
  .wv-tile.on { outline: 3px solid var(--accent, #4F6BFF); outline-offset: 1px; }
  .wv-src { font: 700 clamp(12px, 1.6vw, 20px)/1.1 var(--display, inherit); letter-spacing: -.01em; }
  .wv-meta { font-size: 10.5px; color: #A9B4C2; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .wv-led { position: absolute; top: 8px; right: 8px; width: 6px; height: 6px; border-radius: 50%; background: #3BE07F; box-shadow: 0 0 6px #3BE07F; }
  .wv-tile.noscreen .wv-led { background: #6F7882; box-shadow: none; }
  .wv-base { height: 10px; max-width: 980px; margin: 0 auto; width: 30%; border-radius: 0 0 8px 8px; background: linear-gradient(180deg, #2B3038, #16191D); }
  .wv-form { margin-top: 12px; padding: 14px; border-radius: 14px; background: var(--surface-2); display: grid; gap: 10px; }
  .wv-acts { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; }
</style>
