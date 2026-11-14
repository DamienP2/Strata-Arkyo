<script lang="ts">
  // Mobilier de régie (pupitres, consoles, meubles) : plan de face aux cotes réelles, casquettes
  // rackables posées sur le plateau (glisser pour les déplacer), propriétés, vue 3D. Le
  // matériel d'une casquette se pose dans l'éditeur de racks, comme dans une baie.
  import Icon from '../../components/Icon.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Desk3D from '../../components/Desk3D.svelte';
  import Racks from './Racks.svelte';
  import { api } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { refById } from '../../lib/catalog.svelte';
  import { nf } from '../../lib/format';
  import { ALU, decor, accFace, visualOf } from '../../lib/faceplate';
  import { CONTAINER_KINDS, type Container, type ProjectView } from '../../lib/projects';

  let { view, reload }: { view: ProjectView; reload: () => Promise<void> } = $props();
  const pid = $derived(view.project.id);
  const writable = $derived(can('projets', 'ecriture'));
  const desks = $derived(view.containers.filter((c) => c.kind === 'desk' || c.kind === 'furniture'));
  const cid = $derived(route.path.split('/')[4] ?? null);
  /** Casquette en cours d'équipement : /mobilier/{mobilier}/{casquette}. */
  const equip = $derived(route.path.split('/')[5] ?? null);
  $effect(() => { if (!cid && desks[0]) navigate(`/projets/${pid}/mobilier/${desks[0].id}`, true); });
  const desk = $derived(desks.find((d) => d.id === cid) ?? null);
  const L = $derived(desk?.length_mm ?? 2400), Dp = $derived(desk?.width_mm ?? 900), Hd = $derived(desk?.outer_height_mm ?? 750);
  const casq = $derived(desk ? view.containers.filter((c) => c.kind === 'casquette' && c.parent_id === desk.id).sort((a, b) => (a.offset_mm ?? 0) - (b.offset_mm ?? 0)) : []);
  const WC = 532.6; // largeur d'une casquette : 19″ et ses joues (mm)

  let mode = $state<'plan' | '3d'>((() => { try { return (localStorage.getItem('mob-mode') as 'plan' | '3d') || 'plan'; } catch { return 'plan'; } })());
  $effect(() => { try { localStorage.setItem('mob-mode', mode); } catch { /* indisponible */ } });
  let sel = $state<string | null>(null);
  const selC = $derived(casq.find((c) => c.id === sel) ?? null);

  // Contenu de chaque casquette (même source que l'éditeur de racks).
  interface El { placement: { id: string; face: string; u_start: number; width: string; col: number; shelf_id: string | null }; label: string; tag: string | null; nature: string; height_u: number; function: string | null; accessory: any; item_id: string | null; type_id?: string | null; visual?: string | null; depth_mm?: number | null }
  let racks = $state<Record<string, { elements: El[]; pending: unknown[] }>>({});
  async function loadRacks() {
    const out: Record<string, { elements: El[]; pending: unknown[] }> = {};
    await Promise.all(casq.map(async (c) => { try { out[c.id] = await api.get(`/containers/${c.id}/rack`); } catch { out[c.id] = { elements: [], pending: [] }; } }));
    racks = out;
  }
  $effect(() => { casq.map((c) => `${c.id}:${c.rev}`).join(); loadRacks(); });

  // ----- Plan de face -----
  let planW = $state(900);
  const s = $derived(Math.min(0.55, Math.max(0.15, (planW - 160) / L)));
  const fnColor = (f: string | null) => (f ? (refById('functions', f)?.data?.color ?? 'var(--fg-3)') : 'var(--fg-3)');
  const typeOf = (e: El) => (e.nature === 'accessory' ? accFace(e as any) : visualOf(e));
  const cols = (w: string) => ({ full: 1, half: 2, third: 3, quarter: 4 } as Record<string, number>)[w] ?? 1;
  const topU = $derived(Math.max(6, ...casq.map((c) => c.height_u ?? 6)));
  const planH = $derived((Hd + topU * 44.45 + 120) * s);

  let drag = $state<{ id: string; x0: number; off0: number; off: number } | null>(null);
  function startDrag(e: PointerEvent, c: Container) {
    if (!writable) { sel = c.id; return; }
    sel = c.id;
    drag = { id: c.id, x0: e.clientX, off0: c.offset_mm ?? 0, off: c.offset_mm ?? 0 };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  }
  function moveDrag(e: PointerEvent) {
    if (!drag) return;
    drag = { ...drag, off: Math.round(Math.max(0, Math.min(L - WC, drag.off0 + (e.clientX - drag.x0) / s)) / 10) * 10 };
  }
  async function endDrag() {
    const d = drag; drag = null;
    if (!d || d.off === d.off0) return;
    const c = casq.find((x) => x.id === d.id); if (c) await patch(c, { offset_mm: d.off });
  }
  const offOf = (c: Container) => (drag?.id === c.id ? drag.off : (c.offset_mm ?? 0));

  // ----- Écritures -----
  async function patch(c: Container, changes: Partial<Container>) {
    const { id, project_id: _p, position: _o, rev, totals: _t, ...data } = c;
    try { await api.patch(`/containers/${id}`, { ...data, ...changes, rev }); await reload(); } catch (e) { fail(e); await reload(); }
  }
  async function addCasquette() {
    if (!desk) return;
    // Première place libre de gauche à droite.
    let off = 40;
    for (const c of casq) { if ((c.offset_mm ?? 0) - off >= WC) break; off = Math.max(off, Math.ceil(((c.offset_mm ?? 0) + WC + 20) / 10) * 10); }
    if (off + WC > L) off = Math.max(0, L - WC);
    off = Math.round(off / 10) * 10;
    try {
      const c = await api.post<Container>(`/projects/${pid}/containers`, { location_id: desk.location_id, name: `${desk.name} · casquette ${casq.length + 1}`, kind: 'casquette', rackable: true, height_u: 6, depth_mm: 350, parent_id: desk.id, offset_mm: off, tilt_deg: 15, cable_code: '', details: {} });
      toast('Casquette ajoutée : équipez-la depuis le bouton « Équiper ».'); await reload(); sel = c.id;
    } catch (e) { fail(e); }
  }
  // Nouveau pupitre : on demande l'emplacement et le nom avant de créer.
  let newDesk = $state<{ location_id: string; name: string; busy: boolean } | null>(null);
  function askDesk() {
    if (!view.locations.length) { toast('Créez d’abord un emplacement dans l’inventaire.', 'warn'); return; }
    newDesk = { location_id: view.locations[0]!.id, name: 'Pupitre', busy: false };
  }
  async function addDesk(e: SubmitEvent) {
    e.preventDefault();
    const d = newDesk; if (!d || !d.name.trim() || !d.location_id) return;
    d.busy = true;
    try {
      const c = await api.post<Container>(`/projects/${pid}/containers`, { location_id: d.location_id, name: d.name.trim(), kind: 'desk', rackable: false, length_mm: 2400, width_mm: 900, outer_height_mm: 750, cable_code: '', details: { positions: [] } });
      const loc = view.locations.find((l) => l.id === d.location_id);
      newDesk = null;
      toast(`Mobilier « ${c.name} » ajouté${loc ? ` dans ${loc.name}` : ''}.`);
      await reload(); navigate(`/projets/${pid}/mobilier/${c.id}`);
    } catch (x) { fail(x); if (newDesk) newDesk.busy = false; }
  }
  let confirmDel = $state<Container | null>(null);
  async function delCasquette() {
    const c = confirmDel!; confirmDel = null;
    try { await api.del(`/containers/${c.id}`); toast('Casquette supprimée.'); sel = null; await reload(); } catch (e) { fail(e); }
  }
  const num = (v: string) => (v.trim() === '' ? null : Number(v));
</script>

{#if !desks.length}
  <div class="card empty-mob">
    <span class="kind"><Icon name="layers" /></span>
    <h2>Aucun mobilier dans ce projet</h2>
    <p class="muted">Un mobilier (pupitre, console, meuble) se découpe en postes et reçoit des casquettes rackables pour les écrans, les surfaces de commande et le matériel à portée de main.</p>
    {#if writable}
      {#if view.locations.length}<button class="btn primary" onclick={askDesk}><Icon name="plus" size="sm" />Ajouter un mobilier</button>
      {:else}<p class="muted small">Un mobilier se range dans un emplacement : le projet n’en a pas encore.</p><a class="btn primary" href="/projets/{pid}/inventaire"><Icon name="plus" size="sm" />Créer un emplacement</a>{/if}
    {/if}
  </div>
{:else if desk}
  <div class="mob card" style="padding:0;overflow:hidden">
    <div class="mob-bar">
      <select class="input" aria-label="Mobilier" value={desk.id} onchange={(e) => { sel = null; navigate(`/projets/${pid}/mobilier/${(e.target as HTMLSelectElement).value}`); }}>
        {#each desks as d}<option value={d.id}>{d.name} · {CONTAINER_KINDS[d.kind].label}</option>{/each}
      </select>
      <span class="muted small">{nf(L)} × {nf(Dp)} mm · {casq.length} casquette{casq.length > 1 ? 's' : ''} · {casq.reduce((n, c) => n + (c.height_u ?? 0), 0)} U</span>
      <span style="flex:1"></span>
      {#if equip}
        <div class="seg" role="group" aria-label="Casquette">{#each casq as c (c.id)}<button class:on={equip === c.id} onclick={() => navigate(`/projets/${pid}/mobilier/${desk.id}/${c.id}`)}>{c.name.replace(`${desk.name} · `, '')}</button>{/each}</div>
        <button class="btn sm" onclick={() => navigate(`/projets/${pid}/mobilier/${desk.id}`)}><Icon name="layers" size="sm" />Retour au mobilier</button>
      {:else}
        <div class="seg" role="group" aria-label="Vue"><button class:on={mode === 'plan'} onclick={() => (mode = 'plan')}>Plan</button><button class:on={mode === '3d'} onclick={() => (mode = '3d')}><Icon name="case" size="sm" />3D</button></div>
        {#if writable}<button class="btn sm primary" onclick={addCasquette}><Icon name="plus" size="sm" />Ajouter une casquette</button>{/if}
      {/if}
    </div>
    {#if equip && casq.some((c) => c.id === equip)}
      <!-- Même éditeur que les racks, limité à cette casquette. -->
      {#key equip}<div class="mob-equip"><Racks {view} fixed={equip} /></div>{/key}
    {:else}
    <div class="mob-body">
      <div class="mob-stage">
        {#if mode === '3d'}
          <Desk3D {desk} casquettes={casq.map((c) => ({ c, elements: racks[c.id]?.elements ?? [] }))} />
        {:else}
          <div class="plan" bind:clientWidth={planW} style="height:{planH + 40}px" role="presentation" onpointermove={moveDrag} onpointerup={endDrag}>
            <div class="desk" style="width:{L * s}px;height:{Hd * s}px">
              <span class="top"></span><span class="pad"></span><span class="led"></span>
              {#if casq.length}<span class="deck" style="height:{Math.max(...casq.map((c) => c.height_u ?? 6)) * 44.45 * s * 0.55}px"><i></i></span>{/if}
              <span class="leg l"></span><span class="leg r"></span><span class="voile"></span><span class="tray"></span>
              <span class="floor"></span>
              <span class="dim mono"><i></i>{nf(L)} mm<i></i></span>
              <span class="dimh mono">{nf(Hd)}</span>
              {#each casq as c (c.id)}
                {@const u = (c.height_u ?? 6)}
                {@const up = 44.45 * s}
                <div class="cq" class:sel={sel === c.id} class:dragging={drag?.id === c.id} style="left:{offOf(c) * s}px;width:{WC * s}px;height:{u * up + 50 * s}px;--u:{up}px"
                  role="button" tabindex="0" title="Glisser pour déplacer · double-clic pour équiper" ondblclick={() => navigate(`/projets/${pid}/mobilier/${desk.id}/${c.id}`)} onpointerdown={(e) => startDrag(e, c)} onkeydown={(e) => { if (e.key === 'Enter') sel = c.id; }}>
                  <span class="cq-name">{c.name.replace(`${desk.name} · `, '')}{c.tilt_deg ? ` · ${nf(c.tilt_deg)}°` : ''}</span>
                  {#if (racks[c.id]?.pending.length ?? 0) > 0}<span class="cq-pend" title="Matériel de cette casquette pas encore posé : double-cliquez pour l’équiper">{racks[c.id]!.pending.length} à placer</span>{/if}
                  <div class="cq-in">
                    {#each (racks[c.id]?.elements ?? []).filter((e) => e.placement.face === 'front' && !e.placement.shelf_id) as e (e.placement.id)}
                      {@const n = cols(e.placement.width)}
                      <span class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''}" title="{e.label}{e.tag ? ` · ${e.tag}` : ''}"
                        style="--c:{fnColor(e.function)};bottom:{(e.placement.u_start - 1) * up + 1}px;height:{e.height_u * up - 2}px;left:calc({(100 * e.placement.col) / n}% + 1px);width:calc({100 / n}% - 2px)">{#if up * e.height_u > 16}<span class="rn">{e.label}</span><span class="dec">{@html decor(typeOf(e))}</span>{/if}</span>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
            {#if !casq.length}<p class="plan-hint muted small">Ajoutez une casquette : elle se pose sur le plateau et se glisse à sa place.</p>{/if}
          </div>
        {/if}
      </div>
      <aside class="mob-side">
        {#if selC}
          {@const c = selC}
          <section>
            <span class="lbl-t">Casquette</span>
            <div class="field"><label for="cq-n">Nom</label><input class="input" id="cq-n" value={c.name} disabled={!writable} onchange={(e) => patch(c, { name: (e.target as HTMLInputElement).value })}></div>
            <div class="g2">
              <div class="field"><label for="cq-u">Hauteur (U)</label><input class="input mono" id="cq-u" type="number" min="1" max="24" value={c.height_u ?? 6} disabled={!writable} onchange={(e) => patch(c, { height_u: num((e.target as HTMLInputElement).value) })}></div>
              <div class="field"><label for="cq-d">Profondeur (mm)</label><input class="input mono" id="cq-d" type="number" min="100" max="900" step="10" value={c.depth_mm ?? 350} disabled={!writable} onchange={(e) => patch(c, { depth_mm: num((e.target as HTMLInputElement).value) })}></div>
              <div class="field"><label for="cq-o">Position (mm)</label><input class="input mono" id="cq-o" type="number" min="0" step="10" value={c.offset_mm ?? 0} disabled={!writable} onchange={(e) => patch(c, { offset_mm: num((e.target as HTMLInputElement).value) })}><span class="help">Depuis le bord gauche du plateau.</span></div>
              <div class="field"><label for="cq-t">Inclinaison (°)</label><input class="input mono" id="cq-t" type="number" min="0" max="60" value={c.tilt_deg ?? 0} disabled={!writable} onchange={(e) => patch(c, { tilt_deg: num((e.target as HTMLInputElement).value) })}><span class="help">Vers l’arrière, face vers l’opérateur.</span></div>
            </div>
            <dl class="kv"><dt>Matériel posé</dt><dd>{racks[c.id]?.elements.length ?? 0}</dd><dt>À placer</dt><dd>{racks[c.id]?.pending.length ?? 0}</dd><dt>Code câble</dt><dd class="mono">{c.cable_code}</dd></dl>
            <a class="btn sm primary" href="/projets/{pid}/mobilier/{desk.id}/{c.id}"><Icon name="server" size="sm" />Équiper la casquette</a>
            {#if writable}<button class="btn sm ghost danger" style="justify-self:start" onclick={() => (confirmDel = c)}><Icon name="trash" size="sm" />Supprimer la casquette</button>{/if}
            <button class="btn sm ghost" style="justify-self:start" onclick={() => (sel = null)}>← {desk.name}</button>
          </section>
        {:else}
          <section>
            <span class="lbl-t">{CONTAINER_KINDS[desk.kind].label}</span>
            <h3 class="mob-t">{desk.name}</h3>
            <div class="g2">
              <div class="field"><label for="dk-l">Longueur (mm)</label><input class="input mono" id="dk-l" type="number" min="600" max="3000" step="10" value={desk.length_mm ?? ''} placeholder="2400" disabled={!writable} onchange={(e) => patch(desk, { length_mm: num((e.target as HTMLInputElement).value) })}></div>
              <div class="field"><label for="dk-w">Profondeur (mm)</label><input class="input mono" id="dk-w" type="number" min="300" max="3000" step="10" value={desk.width_mm ?? ''} placeholder="900" disabled={!writable} onchange={(e) => patch(desk, { width_mm: num((e.target as HTMLInputElement).value) })}></div>
              <div class="field"><label for="dk-h">Hauteur (mm)</label><input class="input mono" id="dk-h" type="number" min="400" max="1300" step="10" value={desk.outer_height_mm ?? ''} placeholder="750" disabled={!writable} onchange={(e) => patch(desk, { outer_height_mm: num((e.target as HTMLInputElement).value) })}></div>
            </div>
            <span class="lbl-t" style="margin-top:6px">Casquettes</span>
            {#each casq as c (c.id)}
              <button class="cq-row" onclick={() => (sel = c.id)}><b>{c.name.replace(`${desk.name} · `, '')}</b><span class="muted small mono">{c.height_u} U · {nf(c.offset_mm ?? 0)} mm{c.tilt_deg ? ` · ${nf(c.tilt_deg)}°` : ''}</span><span class="muted small">{racks[c.id]?.elements.length ?? 0} posé{(racks[c.id]?.elements.length ?? 0) > 1 ? 's' : ''}</span></button>
            {:else}<p class="muted small" style="margin:0">Aucune casquette : « Ajouter une casquette » en haut à droite.</p>{/each}
            {#if desk.kind === 'desk'}<p class="muted small" style="margin:6px 0 0">{desk.details?.positions?.length ?? 0} poste{(desk.details?.positions?.length ?? 0) > 1 ? 's' : ''} : rôles et matériel par poste dans l’<a class="ilink" href="/projets/{pid}/inventaire">inventaire</a>.</p>{/if}
          </section>
        {/if}
      </aside>
    </div>
    {/if}
  </div>
{/if}

{#if confirmDel}
  <Confirm title="Supprimer « {confirmDel.name} » ?" eyebrow="Casquette" confirmLabel="Supprimer" danger onconfirm={delCasquette} onclose={() => (confirmDel = null)}>
    <p>Le matériel qu’elle contient est supprimé de l’inventaire avec elle.</p>
  </Confirm>
{/if}

<style>
  .empty-mob { display: grid; justify-items: center; gap: 10px; padding: 48px 24px; text-align: center; }
  .empty-mob .kind { width: 54px; height: 54px; display: grid; place-items: center; border-radius: 14px; background: var(--surface-2); }
  .empty-mob p { max-width: 560px; }
  .mob-equip { padding: 0; }
  .mob-equip :global(.rack-screen) { border-radius: 0; box-shadow: none; }
  .mob-bar { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; padding: 10px 14px; border-bottom: 1px solid var(--line); }
  .mob-bar select { width: auto; min-width: 260px; height: 38px; }
  .mob-body { display: grid; grid-template-columns: minmax(0, 1fr) 320px; }
  .mob-stage { position: relative; min-width: 0; background: radial-gradient(70% 55% at 50% 8%, var(--stage-a) 0%, transparent 100%), linear-gradient(180deg, var(--stage-a), var(--stage-b)); }
  .mob-side { border-left: 1px solid var(--line); padding: 16px; background: var(--surface); overflow: auto; }
  .mob-side section { display: grid; gap: 10px; }
  .mob-t { margin: 0; font-size: 18px; }
  .g2 { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .plan { position: relative; display: flex; align-items: flex-end; justify-content: center; padding: 44px 60px 50px; box-sizing: content-box; min-height: 300px; touch-action: none; }
  .desk { position: relative; flex: none; }
  /* Plateau : noir mat, bandeau noir laqué côté opérateur, repose-poignets cuir, filet LED. */
  .desk .top { position: absolute; left: -2px; right: -2px; top: 0; height: 14px; border-radius: 5px 5px 3px 3px; background: linear-gradient(180deg, #3a3d42 0%, #16171a 40%, #0b0c0e 100%); box-shadow: inset 0 1px 0 rgba(255,255,255,.25), 0 2px 0 #1a1b1e, 0 14px 26px -14px rgba(0,0,0,.65); z-index: 2; }
  .desk .deck { position: absolute; left: 0; right: 0; bottom: 100%; border-radius: 6px 6px 0 0; background: linear-gradient(180deg, #2a2c31, #111215); box-shadow: inset 0 1px 0 rgba(255,255,255,.08); z-index: 1; }
  .desk .deck i { position: absolute; left: 3%; right: 3%; top: 3px; height: 2px; border-radius: 2px; background: #9cc8ff; box-shadow: 0 0 6px 1px rgba(90,168,255,.8); }
  .desk .pad { position: absolute; left: 8%; right: 8%; top: -6px; height: 8px; border-radius: 6px; background: linear-gradient(180deg, #34373d, #141517); box-shadow: 0 1px 2px rgba(0,0,0,.5); z-index: 3; }
  .desk .led { position: absolute; left: 4%; right: 4%; top: 15px; height: 2px; border-radius: 2px; background: #9cc8ff; box-shadow: 0 0 6px 1px rgba(90,168,255,.85), 0 0 18px 4px rgba(90,168,255,.35); z-index: 2; }
  .desk .leg { position: absolute; top: 12px; bottom: 0; width: 20px; border-radius: 3px; background: linear-gradient(90deg, #8c939b, #e3e6ea 35%, #b9bec5 60%, #7f868e); box-shadow: inset 0 0 0 1px rgba(0,0,0,.25); z-index: 1; }
  .desk .leg.l { left: 0; } .desk .leg.r { right: 0; }
  .desk .voile { position: absolute; left: 28px; right: 28px; top: 14px; height: 42%; border-radius: 0 0 4px 4px; background: linear-gradient(180deg, #2a2d33, #202328); box-shadow: inset 0 -1px 0 rgba(255,255,255,.06); }
  .desk .tray { position: absolute; left: 30px; right: 30px; bottom: 16%; height: 5px; border-radius: 2px; background: linear-gradient(180deg, #c9ccd1, #8b9097); opacity: .8; }
  .desk .floor { position: absolute; left: -4%; right: -4%; bottom: -10px; height: 14px; border-radius: 50%; background: radial-gradient(50% 50% at 50% 50%, rgba(0,0,0,.28), transparent 70%); z-index: 0; }
  .desk .dim { position: absolute; left: 0; right: 0; bottom: -30px; display: flex; align-items: center; gap: 8px; font-size: 11px; color: var(--fg-3); }
  .desk .dim i { flex: 1; height: 1px; background: var(--line-2); position: relative; }
  .desk .dim i:first-child::before, .desk .dim i:last-child::after { content: ''; position: absolute; top: -4px; width: 1px; height: 9px; background: var(--fg-3); }
  .desk .dim i:first-child::before { left: 0; } .desk .dim i:last-child::after { right: 0; }
  .desk .dimh { position: absolute; left: -48px; top: 50%; transform: translateY(-50%); font-size: 11px; color: var(--fg-3); }
  /* Casquette encastrée : joues noir laqué, dessus noir, liseré alu ; gradin continu derrière. */
  .cq { position: absolute; bottom: 100%; margin-bottom: 0; padding: 0 calc(25 * var(--u) / 44.45) calc(25 * var(--u) / 44.45); box-sizing: border-box; border-radius: 7px 7px 2px 2px; background: linear-gradient(90deg, #050506, #2a2c31 5%, #141518 6.5%, #141518 93.5%, #2a2c31 95%, #050506); box-shadow: inset 0 3px 0 #1b1d21, inset 0 4px 0 rgba(220,224,230,.6), 0 0 0 1px #0d0e10, 0 14px 22px -14px rgba(0,0,0,.75); cursor: grab; touch-action: none; z-index: 2; }
  .cq.sel { box-shadow: inset 0 3px 0 #1b1d21, inset 0 4px 0 rgba(220,224,230,.6), 0 0 0 1.5px var(--accent), 0 14px 22px -14px rgba(0,0,0,.75); }
  .cq.dragging { cursor: grabbing; opacity: .9; }
  .cq-name { position: absolute; left: 0; right: 0; bottom: 100%; margin-bottom: 4px; text-align: center; font: 600 11px var(--font); color: var(--fg-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cq-in { position: absolute; left: calc(25 * var(--u) / 44.45 + 10px); right: calc(25 * var(--u) / 44.45 + 10px); top: calc(25 * var(--u) / 44.45); bottom: calc(25 * var(--u) / 44.45); background: repeating-linear-gradient(to top, #1E2228 0 1px, #08090B 1px var(--u)); }
  .cq-in .ritem { position: absolute; font-size: 11px; padding: 0 5px; }
  .cq-pend { position: absolute; left: 50%; bottom: 100%; margin-bottom: 22px; transform: translateX(-50%); white-space: nowrap; font: 600 10.5px/1.4 var(--mono); padding: 1px 6px; border-radius: 4px; background: var(--warn, #B78400); color: #fff; z-index: 3; }
  /* Le matériel posé au-delà de la hauteur de la casquette ne déborde pas sur le plateau. */
  .cq-in { overflow: hidden; }
  .plan-hint { position: absolute; left: 0; right: 0; top: 30px; text-align: center; }
  .cq-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 2px 8px; padding: 8px 10px; border: 0; border-radius: 10px; background: var(--surface-2); text-align: left; font: inherit; color: inherit; cursor: pointer; }
  .cq-row:hover { box-shadow: inset 0 0 0 1.5px var(--accent); }
  .cq-row .muted.small:last-child { grid-column: 1 / -1; }
  @media (max-width: 1000px) { .mob-body { grid-template-columns: minmax(0, 1fr); } .mob-side { border-left: 0; border-top: 1px solid var(--line); } }
</style>

{#if newDesk}
  <Dialog title="Ajouter un mobilier" eyebrow="Mobilier" onclose={() => (newDesk = null)}>
    <form id="deskf" class="stack" onsubmit={addDesk}>
      <div class="field"><label for="nd-loc">Emplacement</label><select class="input" id="nd-loc" bind:value={newDesk.location_id}>{#each view.locations as l}<option value={l.id}>{l.name}{l.code && l.code !== l.name ? ` · ${l.code}` : ''}</option>{/each}</select></div>
      <div class="field"><label for="nd-name">Nom</label><input class="input" id="nd-name" bind:value={newDesk.name} required maxlength="120" placeholder="Pupitre réalisateur, console son…"></div>
      <p class="muted small">Mobilier de 2 400 × 900 mm, 750 mm de haut : les cotes se règlent ensuite dans le panneau de droite.</p>
    </form>
    {#snippet footer()}<button class="btn ghost" onclick={() => (newDesk = null)}>Annuler</button><button class="btn primary" type="submit" form="deskf" disabled={!newDesk?.name.trim() || newDesk?.busy}>{#if newDesk?.busy}<span class="spin"></span>{/if}Ajouter le mobilier</button>{/snippet}
  </Dialog>
{/if}
