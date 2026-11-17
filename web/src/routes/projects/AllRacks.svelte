<script lang="ts">
  // Toutes les baies côte à côte, entre les mêmes panneaux que la vue d'un rack : « À placer »
  // à gauche (par baie), détail de l'élément choisi ou totaux à droite. Un équipement glissé
  // d'une baie à une autre change d'élément dans l'inventaire (un exemplaire à la fois) et y est
  // posé à l'endroit lâché ; dans la même baie, il change simplement de position. Un élément « à
  // placer » glissé dans une baie y est posé (et y est déplacé s'il appartenait à une autre).
  // La main est prise le temps du geste.
  import Icon from '../../components/Icon.svelte';
  import PanelFace from '../../components/PanelFace.svelte';
  import Seg from '../../components/Seg.svelte';
  import Vu from '../../components/Vu.svelte';
  import Inspector from '../../components/Inspector.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { navigate } from '../../lib/router.svelte';
  import { familyColor } from '../../lib/catalog.svelte';
  import { ALU, decor, accFace, visualOf } from '../../lib/faceplate';
  import { rackLines, modelOf } from '../../lib/racktext.svelte';
  import { canvasKeys } from '../../lib/canvaskeys';
  import { blankClick } from '../../lib/blankclick';
  import { nf, pl } from '../../lib/format';
  import { fmtU, isWhole, occTxt, rangeTxt } from '../../lib/rackgeom';
  import type { Accessory, ProjectView } from '../../lib/projects';

  import RackPicker from '../../components/RackPicker.svelte';
  let { view, reload, loc = null }: { view: ProjectView; reload?: () => Promise<void>; loc?: string | null } = $props();
  type Face = 'front' | 'rear';
  interface El { placement: { id: string; article_id: string; instance_id: string | null; face: string; u_start: number; width: string; col: number; shelf_id: string | null; circuit_id?: string | null; rev: number }; label: string; tag: string | null; custom?: string | null; brand?: string | null; nature: string; height_u: number; depth_mm?: number | null; weight_kg?: number | null; power_w?: number | null; function: string | null; accessory: Accessory | null; article_rev: number; type_id?: string | null; visual?: string | null; item_id?: string | null }
  interface Pend { article_id: string; article_rev?: number; instance_id?: string; tag?: string; label: string; nature: string; height_u: number | null; width: string | null; rackable: boolean; function: string | null }
  interface R {
    container: any; elements: El[]; pending: Pend[]; warnings: { placement_id: string; message: string }[];
    totals: { placed_u?: number; used_u: number; total_u: number | null; weight_kg: number; power_w: number; over_weight: boolean; over_power: boolean; over_u: boolean };
    lock: { mine: boolean; account_name: string } | null;
  }

  // Un emplacement choisi (ou tous), rangés par emplacement.
  const racks = $derived(view.locations.flatMap((l) => (loc && l.id !== loc ? [] : view.containers.filter((c) => c.rackable && c.kind !== 'casquette' && c.location_id === l.id))));
  let data = $state<Record<string, R>>({});
  let face = $state<Face>('front');
  let scale = $state(24);
  let busy = $state(false);
  const canDesign = $derived(can('conception', 'ecriture'));

  async function loadAll() {
    const out: Record<string, R> = {};
    await Promise.all(racks.map(async (c) => { try { out[c.id] = await api.get<R>(`/containers/${c.id}/rack`); } catch (e) { fail(e); } }));
    data = out;
  }
  $effect(() => { racks.length; loadAll(); });

  const cols = (w: string) => ({ full: 1, half: 2, third: 3, quarter: 4 } as Record<string, number>)[w] ?? 1;
  const famColor = (f: string | null) => familyColor(f, { screen: true });
  const withRacks = $derived(view.locations.filter((l) => view.containers.some((c) => c.rackable && c.kind !== 'casquette' && c.location_id === l.id)));
  const typeOf = (e: El) => (e.nature === 'accessory' ? accFace(e as any) : visualOf(e));
  const panelOf = (e: El) => (e.accessory?.kind === 'panel' ? e.accessory.panel : null);
  const nameOf = (e: { custom?: string | null; tag?: string | null; label: string }) => e.custom || e.tag || e.label;
  function style(e: El) {
    const n = cols(e.placement.width), l = (100 * e.placement.col) / n, w = 100 / n;
    return `--c:${famColor(e.function)};bottom:calc(${e.placement.u_start - 1} * var(--u) + 1px);height:calc(${e.height_u} * var(--u) - 2px);left:calc(${l}% + 2px);width:calc(${w}% - 4px)`;
  }
  const FACE: Record<string, string> = { front: 'Avant', rear: 'Arrière', rear_inner: 'Arrière intérieure' };

  // ----- Sélection : détail à droite ; Échap et clic dans le vide désélectionnent -----
  let sel = $state<{ cid: string; id: string } | null>(null);
  const selEl = $derived(sel ? (data[sel.cid]?.elements.find((e) => e.placement.id === sel!.id) ?? null) : null);
  const selRack = $derived(sel ? (view.containers.find((c) => c.id === sel!.cid) ?? null) : null);
  $effect(() => canvasKeys({ onEscape: () => { if (sel) sel = null; } }));
  $effect(() => blankClick({ enabled: () => !!sel && !drag, onBlank: () => (sel = null) }));

  // ----- Glisser-déposer -----
  let drag = $state<{ from: string; e?: El; p?: Pend } | null>(null);
  let over = $state<{ cid: string; u: number } | null>(null);
  const dragH = () => (drag?.e ? drag.e.height_u : drag?.p?.height_u) || 1;
  /** U visé (au demi-U pour un élément d'un demi-U), centré sur la prise, dans les limites. */
  function uAt(ev: DragEvent, el: HTMLElement, units: number) {
    const r = el.getBoundingClientRect(), h = dragH(), st = isWhole(h) ? 1 : 0.5;
    const grab = Math.floor(h / 2 / st) * st;
    const u = Math.floor(((r.bottom - ev.clientY) / scale - grab) / st + 1e-9) * st + 1;
    return Math.max(1, Math.min(units - h + 1, u));
  }
  async function lock(cid: string): Promise<boolean> {
    const r = await api.post<{ mine: boolean; account_name: string }>(`/containers/${cid}/rack/lock`);
    if (!r.mine) toast(`${r.account_name} modifie « ${data[cid]?.container.name} » : réessayez plus tard.`, 'warn');
    return r.mine;
  }
  async function unlock(cid: string) { try { await api.del(`/containers/${cid}/rack/lock`); } catch { /* déjà libre */ } }
  /** Prend la main sur les baies du geste, exécute, puis rend la main et recharge. */
  async function withLocks(ids: string[], run: () => Promise<void>) {
    if (busy) return;
    busy = true;
    const locked: string[] = [];
    try {
      for (const id of new Set(ids)) { if (!(await lock(id))) return; locked.push(id); }
      await run();
    } catch (x) { fail(x); }
    finally {
      for (const id of locked) await unlock(id);
      busy = false;
      await loadAll();
      await reload?.();
    }
  }
  async function dropOn(cid: string, u: number) {
    const d = drag; drag = null; over = null;
    if (!d || busy) return;
    const dest = view.containers.find((c) => c.id === cid)!;
    if (d.p) {
      const p = d.p;
      await withLocks([cid], async () => {
        let article = p.article_id;
        if (d.from !== cid) {
          const moved = await api.post<{ id: string }>(`/articles/${p.article_id}/move`, { rev: p.article_rev, container_id: cid, location_id: dest.location_id, instance_id: p.instance_id ?? null });
          article = moved.id;
        }
        try {
          const r = await api.post<{ placement: { id: string; u_start: number } }>(`/containers/${cid}/placements`, { article_id: article, instance_id: p.instance_id ?? null, face, u_start: u, col: 0 });
          toast(`« ${p.tag ?? p.label} » posé dans « ${dest.name} » en ${rangeTxt(r.placement.u_start, p.height_u || 1)}.`);
          sel = { cid, id: r.placement.id };
        } catch (x) { toast(`« ${p.tag ?? p.label} » est dans « ${dest.name} », à placer : ${(x as Error).message}`, 'warn'); }
      });
      return;
    }
    const e = d.e!;
    if (d.from === cid && e.placement.u_start === u && e.placement.face === face) return;
    await withLocks([d.from, cid], async () => {
      if (d.from === cid) {
        await api.patch(`/placements/${e.placement.id}`, { rev: e.placement.rev, face, u_start: u, width: e.placement.width, col: e.placement.col, circuit_id: null });
      } else {
        // Retirer de la baie d'origine, changer d'élément dans l'inventaire, poser dans la nouvelle.
        await api.del(`/placements/${e.placement.id}`);
        const moved = await api.post<{ id: string }>(`/articles/${e.placement.article_id}/move`, { rev: e.article_rev, container_id: cid, location_id: dest.location_id, instance_id: e.placement.instance_id });
        try {
          const r = await api.post<{ adjusted: boolean; placement: { id: string; u_start: number } }>(`/containers/${cid}/placements`, { article_id: moved.id, instance_id: e.placement.instance_id, face, u_start: u, col: 0 });
          toast(`${e.label} déplacé dans « ${dest.name} »${r.adjusted ? `, en U${fmtU(r.placement.u_start)}` : ''}.`);
          if (sel?.id === e.placement.id) sel = { cid, id: r.placement.id };
        } catch (x) {
          toast(`${e.label} est dans « ${dest.name} », à placer : ${(x as Error).message}`, 'warn');
        }
      }
    });
  }
  /** Retire l'élément choisi de sa baie (il redevient « à placer »), avec Annuler. */
  async function unplace(cid: string, e: El) {
    const p = e.placement;
    await withLocks([cid], async () => {
      await api.del(`/placements/${p.id}`);
      sel = null;
      toast(`« ${e.tag ?? e.label} » retiré du rack : il est à placer.`, 'ok', { run: () => withLocks([cid], async () => {
        await api.post(`/containers/${cid}/placements`, { article_id: p.article_id, instance_id: p.instance_id, face: p.face, u_start: p.u_start, width: p.width, col: p.col, circuit_id: p.circuit_id ?? null });
      }) });
    });
  }

  // ----- À placer (par baie) et totaux -----
  const pendOf = (cid: string) => (data[cid]?.pending ?? []).filter((p) => p.rackable).sort((a, b) => a.label.localeCompare(b.label, 'fr') || (a.tag ?? '').localeCompare(b.tag ?? ''));
  const nPend = $derived(racks.reduce((n, c) => n + pendOf(c.id).length, 0));
  const placedOf = (c: (typeof racks)[number]) => data[c.id]?.totals.placed_u ?? c.totals.placed_u ?? c.totals.used_u;
  const sum = $derived(racks.reduce((s, c) => { const t = data[c.id]?.totals; return { u: s.u + placedOf(c), total: s.total + (c.height_u ?? 0), kg: s.kg + (t?.weight_kg ?? 0), w: s.w + (t?.power_w ?? 0) }; }, { u: 0, total: 0, kg: 0, w: 0 }));
</script>

<div class="rack-screen card solo all" style="padding:0;overflow:hidden">
  <div class="dtool rk-tool">
    <div class="tg tg-pick"><RackPicker {view} value={loc ? `tout:${loc}` : 'tout'} onpick={(v) => navigate(`/projets/${view.project.id}/racks/${v.replace(':', '/')}`)} /></div>
    {#if withRacks.length > 1}
      <!-- Filtre d'emplacement : des liens (l'emplacement choisi est dans l'adresse). -->
      <div class="tg"><nav class="seg ar-locs" aria-label="Emplacement">
        <a href="/projets/{view.project.id}/racks/tout" class:on={!loc} aria-current={!loc ? 'page' : undefined}>Tous</a>
        {#each withRacks as l (l.id)}<a href="/projets/{view.project.id}/racks/tout/{l.id}" class:on={loc === l.id} aria-current={loc === l.id ? 'page' : undefined}>{l.name}</a>{/each}
      </nav></div>
    {/if}
    <div class="tg">
      <Seg label="Face" bind:value={face} options={[{ value: 'front', label: 'Avant' }, { value: 'rear', label: 'Arrière' }]} />
      <Seg label="Taille" bind:value={scale} options={[{ value: 18, label: 'S', title: 'Petite taille' }, { value: 24, label: 'M', title: 'Taille moyenne' }, { value: 32, label: 'L', title: 'Grande taille' }]} />
      {#if busy}<span class="spin" role="status" aria-label="Enregistrement"></span>{/if}
    </div>
    <div class="tg tg-meters">
      <span class="meter" title="U posés dans les racks affichés"><span>U</span><Vu value={(sum.u / (sum.total || 1)) * 100} mode="load" n={16} /><span class="mono">{fmtU(sum.u)} / {sum.total} U</span></span>
      <span class="meter"><span>Poids</span><span class="mono">{nf(sum.kg, 1)} kg</span></span>
      <span class="meter"><span>Puiss.</span><span class="mono">{nf(sum.w)} W</span></span>
    </div>
  </div>

  <div class="design-body">
    <aside class="pane left" aria-label="À placer">
      <section>
        <h2>À placer <span class="muted">· {nPend}</span></h2>
        <p class="small muted">{#if canDesign}Glissez un élément dans un rack pour l’y poser ; vers un autre rack, il change d’élément dans l’inventaire.{:else}Lecture seule.{/if}</p>
        {#each racks as c (c.id)}
          {@const list = pendOf(c.id)}
          {#if list.length}
            <div class="pcat eyebrow"><span>{c.name}</span><b>{list.length}</b></div>
            {#each list as p (p.instance_id ?? p.article_id)}
              <div class="pend mini" class:dragging={drag?.p === p} role="button" tabindex="0" draggable={canDesign && !busy} title="{p.label}{p.tag ? ` · ${p.tag}` : ''} : glissez-le dans un rack"
                aria-label="{p.label}{p.tag ? ` · ${p.tag}` : ''}, {fmtU(p.height_u ?? 1)} U, à placer dans {c.name}"
                ondragstart={(ev) => { drag = { from: c.id, p }; ev.dataTransfer?.setData('text/plain', p.label); }} ondragend={() => { drag = null; over = null; }}>
                <i class="pm-c" style="--c:{famColor(p.function)}"></i><span class="pm-l">{p.label}</span>{#if p.tag}<span class="pm-t">{p.tag}</span>{/if}<span class="pm-u">{fmtU(p.height_u ?? 0)} U</span>
              </div>
            {/each}
          {/if}
        {:else}<p class="small muted">Aucun rack.</p>{/each}
        {#if !nPend && racks.length}<p class="small muted">Tout est posé.</p>{/if}
      </section>
    </aside>

    <div class="rack-stage ar-stage">
      <div class="all-racks">
        {#each racks as c (c.id)}
          {@const r = data[c.id]}
          {@const units = c.height_u ?? 0}
          <div class="ar-col">
            <div class="ar-head"><b>{c.name}</b><span class="muted small">{view.locations.find((l) => l.id === c.location_id)?.name} · {units} U</span>
              {#if r?.lock && !r.lock.mine}<span class="chip warn"><Icon name="lock" />{r.lock.account_name}</span>{/if}
              <a class="icon-btn sm" href="/projets/{view.project.id}/racks/{c.id}" aria-label="Ouvrir le rack {c.name}" title="Ouvrir ce rack"><Icon name="arrowr" size="sm" /></a></div>
            <div class="rack" style="--units:{units};--u:{scale}px;--bw:{Math.round(scale * 10.86) - 22}px">
              <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
              <div class="rack-body" role="grid" tabindex="-1" aria-label="Élévation de {c.name}" class:target={over?.cid === c.id}
                ondragover={(ev) => { if (drag) { ev.preventDefault(); over = { cid: c.id, u: uAt(ev, ev.currentTarget as HTMLElement, units) }; } }}
                ondragleave={() => { if (over?.cid === c.id) over = null; }}
                ondrop={(ev) => { ev.preventDefault(); if (drag) dropOn(c.id, uAt(ev, ev.currentTarget as HTMLElement, units)); }}>
                {#each (r?.elements ?? []).filter((e) => e.placement.face === face && !e.placement.shelf_id) as e (e.placement.id)}
                  <div class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''} {e.placement.width !== 'full' ? `half c${e.placement.col}` : ''}" class:dragged={drag?.e === e} class:draggable={canDesign && !busy} class:sel={sel?.id === e.placement.id}
                    style={style(e)} role="button" tabindex="0" draggable={canDesign && !busy} aria-pressed={sel?.id === e.placement.id} title="{e.custom ? `${e.custom} · ` : ''}{e.tag ? `${e.tag} · ` : ''}{e.label} · U{fmtU(e.placement.u_start)}"
                    onclick={() => (sel = { cid: c.id, id: e.placement.id })} onkeydown={(k) => { if (k.key === 'Enter' || k.key === ' ') { k.preventDefault(); sel = { cid: c.id, id: e.placement.id }; } }}
                    ondragstart={(ev) => { drag = { from: c.id, e }; ev.dataTransfer?.setData('text/plain', e.placement.id); }} ondragend={() => { drag = null; over = null; }}>
                    {#if e.nature !== 'accessory'}<i class="led"></i>{/if}<span class="rl" class:two={!!rackLines(e)[1]}>{#if rackLines(e)[0]}<b class="rn" class:id={!!e.tag && rackLines(e)[0] === e.tag}>{rackLines(e)[0]}</b>{/if}{#if rackLines(e)[1]}<small>{rackLines(e)[1]}</small>{/if}</span>{#if panelOf(e)}<span class="pdec"><PanelFace panel={panelOf(e)!} /></span>{:else}<span class="dec">{@html decor(typeOf(e))}</span>{/if}
                  </div>
                {/each}
                {#if drag && over?.cid === c.id}
                  <div class="rghost" style="bottom:calc({over.u - 1} * var(--u));height:calc({dragH()} * var(--u));left:2px;width:calc(100% - 4px)"></div>
                {/if}
              </div>
              <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
            </div>
          </div>
        {/each}
      </div>
    </div>

    <aside class="pane right">
      {#if selEl && sel && selRack}
        {@const e = selEl}
        {@const p = e.placement}
        <Inspector eyebrow={e.nature === 'accessory' ? 'Accessoire' : 'Équipement'} title={e.custom || e.label} id={e.tag} sub={e.custom ? modelOf(e) : null} onclose={() => (sel = null)}>
          {#each (data[sel.cid]?.warnings ?? []).filter((w) => w.placement_id === p.id) as w}<div class="alert warn"><Icon name="alert" /><span>{w.message}</span></div>{/each}
          <section class="insp-sec">
            <h3>Position</h3>
            <div class="insp-row"><span class="insp-k">Rack</span><span class="insp-v">{selRack.name}</span></div>
            <div class="insp-row"><span class="insp-k">U de départ</span><span class="insp-v mono">U{fmtU(p.u_start)}</span></div>
            <div class="insp-row"><span class="insp-k">Occupe</span><span class="insp-v">{occTxt(p.u_start, e.height_u || 1)}</span></div>
            <div class="insp-row"><span class="insp-k">Face</span><span class="insp-v">{FACE[p.face] ?? p.face}</span></div>
            <div class="insp-row"><span class="insp-k">Hauteur</span><span class="insp-v">{fmtU(e.height_u)} U</span></div>
          </section>
          {#if e.nature !== 'accessory' || e.weight_kg || e.depth_mm}
            <section class="insp-sec">
              <h3>Caractéristiques</h3>
              <div class="insp-row"><span class="insp-k">Poids</span><span class="insp-v" class:muted={!e.weight_kg}>{e.weight_kg ? `${nf(e.weight_kg, 1)} kg` : 'Non renseigné'}</span></div>
              {#if e.nature !== 'accessory'}<div class="insp-row"><span class="insp-k">Puissance</span><span class="insp-v" class:muted={!e.power_w}>{e.power_w ? `${nf(e.power_w)} W` : 'Non renseignée'}</span></div>{/if}
              <div class="insp-row"><span class="insp-k">Profondeur</span><span class="insp-v" class:muted={!e.depth_mm}>{e.depth_mm ? `${nf(e.depth_mm)} mm` : 'Non renseignée'}</span></div>
            </section>
          {/if}
          {#snippet actions()}
            <a class="btn sm ghost" href="/projets/{view.project.id}/racks/{sel!.cid}{p.instance_id ? `?i=${p.instance_id}` : ''}"><Icon name="arrowr" size="sm" />Ouvrir dans le rack</a>
            {#if e.item_id && e.nature !== 'accessory'}<a class="btn sm ghost" href="/catalogue/equipment/{e.item_id}"><Icon name="file" size="sm" />Ouvrir la fiche</a>{/if}
            {#if canDesign}<button class="btn sm ghost danger" disabled={busy} title="Il redevient à placer" onclick={() => unplace(sel!.cid, e)}><Icon name="x" size="sm" />Retirer du rack</button>{/if}
          {/snippet}
        </Inspector>
      {:else}
        <section>
          <h2>Totaux</h2>
          <dl class="kv"><dt>Racks</dt><dd>{racks.length}</dd><dt>Hauteur posée</dt><dd>{fmtU(sum.u)} / {sum.total} U</dd><dt>Poids</dt><dd>{nf(sum.kg, 1)} kg</dd><dt>Puissance</dt><dd>{nf(sum.w)} W</dd><dt>Dissipation</dt><dd>{nf(sum.w * 3.412)} BTU/h</dd>{#if nPend}<dt>À placer</dt><dd>{nPend} élément{pl(nPend)}</dd>{/if}</dl>
        </section>
        <section>
          <h2>Par rack</h2>
          {#each racks as c (c.id)}
            {@const t = data[c.id]?.totals}
            <a class="ar-tot" href="/projets/{view.project.id}/racks/{c.id}" title="Ouvrir le rack {c.name}">
              <span class="ar-tn"><b>{c.name}</b><small>{nf(t?.weight_kg ?? 0, 1)} kg · {nf(t?.power_w ?? 0)} W{(data[c.id]?.warnings.length ?? 0) ? ` · ${data[c.id]!.warnings.length} alerte${pl(data[c.id]!.warnings.length)}` : ''}</small></span>
              <span class="ar-tu mono" class:err={t?.over_u}>{fmtU(placedOf(c))} / {c.height_u} U</span>
            </a>
          {/each}
        </section>
      {/if}
    </aside>
  </div>
</div>

<style>
  .ar-stage { padding: 12px; display: block; }
  .all-racks { display: flex; gap: 22px; align-items: flex-start; justify-content: safe center; min-width: min-content; padding: 4px 6px 12px; }
  .ar-col { flex: none; display: flex; flex-direction: column; gap: 8px; }
  .ar-col :global(.rack-body) { width: var(--bw) !important; }
  .ar-col :global(.rack) { grid-template-columns: 22px auto 22px; }
  .ar-head { display: flex; align-items: center; gap: 8px; padding: 0 4px; max-width: calc(var(--bw, 300px) + 60px); }
  .ar-head b { font: 650 var(--fs-md, 13.5px) var(--display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ar-head .muted { margin-right: auto; white-space: nowrap; }
  .rack-body.target { outline: 2px dashed var(--accent); outline-offset: 2px; }
  .ritem.dragged { opacity: .35; }
  .ritem.draggable { cursor: grab; }
  /* Liens du filtre d'emplacement : même aspect que les boutons d'une commande segmentée. */
  .ar-locs a { display: inline-flex; align-items: center; height: var(--btn-h-sm, 30px); padding: 0 11px; border-radius: var(--btn-r-sm, 7px); font-size: var(--fs-ctl-sm, 13px); font-weight: 550; color: var(--fg-2); text-decoration: none; white-space: nowrap; }
  .ar-locs a:hover { color: var(--fg); }
  .ar-locs a.on { background: var(--key-bg); color: var(--fg); font-weight: 650; box-shadow: var(--key); }
  .ar-locs a:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .pdec { flex: 1 1 0; min-width: 0; align-self: stretch; display: block; padding: 2px 0; margin-left: 8px; }
  /* À placer : mêmes lignes que la vue d'un rack. */
  .pcat { display: flex; align-items: center; gap: 8px; margin: 10px 0 2px; padding: 0 2px; }
  .pcat span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pcat b { margin-left: auto; font: 600 11px var(--mono); padding: 1px 7px; border-radius: 99px; background: var(--surface-3); color: var(--fg-2); }
  .pend.mini { display: flex; flex-direction: row; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px; min-height: 34px; }
  .pend.mini[draggable='true'] { cursor: grab; }
  .pend.mini.dragging { opacity: .4; }
  .pend.mini:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .pm-c { flex: none; width: 4px; align-self: stretch; border-radius: 2px; background: var(--c); }
  .pm-l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13.5px !important; font-weight: 600; color: var(--fg) !important; }
  .pm-t { flex: none; font: 600 11px/1 var(--mono) !important; color: var(--fg-3); }
  .pm-u { flex: none; font: 600 11px/1 var(--mono) !important; padding: 3px 6px; border-radius: 5px; background: var(--sunken); color: var(--fg-2) !important; }
  /* Totaux par rack : une ligne cliquable par rack. */
  .ar-tot { display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: var(--r-sm, 8px); color: var(--fg); text-decoration: none; border-bottom: 1px solid var(--line); }
  .ar-tot:hover { background: color-mix(in srgb, var(--accent) 7%, transparent); }
  .ar-tot:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .ar-tn { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.3; }
  .ar-tn b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ar-tn small { font-size: var(--fs-sm, 12.5px); color: var(--fg-3); }
  .ar-tu { flex: none; font-size: 12.5px; color: var(--fg-2); }
  .ar-tu.err { color: var(--err); font-weight: 700; }
  @media (max-width: 767px) { .rack-screen.all .design-body { display: flex; flex-direction: column; } }
</style>
