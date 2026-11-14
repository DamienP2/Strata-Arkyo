<script lang="ts">
  // Élévations de racks (section 9) : écran de conception, une personne à la fois.
  import { untrack, onDestroy } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Vu from '../../components/Vu.svelte';
  import Confirm from '../../components/Confirm.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can, session } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import { route, navigate } from '../../lib/router.svelte';
  import { join } from '../../lib/live.svelte';
  import { nf, relative } from '../../lib/format';
  import { refById, loadRefs } from '../../lib/catalog.svelte';
  import { rackLines, rackText, setRackText, RACK_TEXT, type RackText } from '../../lib/racktext.svelte';
  import { faceType, ALU, decor, accFace, visualOf } from '../../lib/faceplate';
  import { CONTAINER_KINDS, type Accessory, type Article, type ProjectView } from '../../lib/projects';
  import PanelFace from '../../components/PanelFace.svelte';
  import ProjectPanel from './ProjectPanel.svelte';
  import Rack3D from '../../components/Rack3D.svelte';
  import AllRacks from './AllRacks.svelte';
  import RackPicker from '../../components/RackPicker.svelte';

  // `fixed` : éditeur d'un seul élément, intégré à une autre page (casquette d'un mobilier).
  let { view, fixed = null }: { view: ProjectView; fixed?: string | null } = $props();
  type Face = 'front' | 'rear' | 'rear_inner';
  type W = 'full' | 'half' | 'third' | 'quarter';
  interface Placement { id: string; article_id: string; instance_id: string | null; face: Face; u_start: number; width: W; col: number; circuit_id: string | null; shelf_id: string | null; rev: number }
  interface RackEl { placement: Placement; label: string; tag: string | null; custom?: string | null; nature: string; height_u: number; depth_mm: number | null; rear_reserve_mm: number | null; weight_kg: number | null; power_w: number | null; function: string | null; category: string | null; brand: string | null; accessory: Accessory | null; article_rev: number; type_id?: string | null; visual?: string | null }
  interface Pending { article_id: string; instance_id?: string; tag?: string; label: string; nature: string; height_u: number | null; width: W | null; rackable: boolean; function: string | null; accessory: Accessory | null; type_id?: string | null; visual?: string | null }
  interface VPdu { article_id: string; rev: number; label: string; accessory: { kind: 'vertical_pdu'; length_mm: number; outlets: number; side: 'left' | 'right'; circuit_id?: string | null }; circuit: string | null; fed: number; free: number; over: boolean }
  interface Circuit { id: string; name: string; phase: 'L1' | 'L2' | 'L3'; rating_a: number; rev: number }
  interface Rack {
    container: any; elements: RackEl[]; pending: Pending[]; circuits: Circuit[]; faces: Face[]; vpdus?: VPdu[];
    electrical: { circuits: { id: string; name: string; phase: string; rating_a: number; watts: number; amps: number; over: boolean }[]; phases: { phase: string; watts: number; amps: number }[]; imbalance_pct: number; unbalanced: boolean; unassigned_watts: number };
    totals: { used_u: number; total_u: number | null; weight_kg: number; power_w: number; over_weight: boolean; over_power: boolean; over_u: boolean };
    warnings: { placement_id: string; message: string }[];
    lock: { account_id: string; account_name: string; taken_at: string; mine: boolean } | null;
  }

  loadRefs();
  // Les casquettes s'équipent depuis le mobilier : elles ne figurent pas dans les racks.
  const racks = $derived(view.containers.filter((c) => c.rackable && c.kind !== 'casquette'));
  const all = $derived(!fixed && route.path.split('/')[4] === 'tout');
  const cid = $derived(fixed ?? (all ? null : (route.path.split('/')[4] ?? racks[0]?.id ?? null)));
  $effect(() => { if (!fixed && !route.path.split('/')[4] && racks[0]) navigate(`/projets/${view.project.id}/racks/${racks[0].id}`, true); });

  let rack = $state<Rack | null>(null);
  let face = $state<Face | 'side'>('front');
  let mode = $state<'real' | 'schematic'>('real');
  let show3d = $state(false);
  let armed = $state<Pending | null>(null);
  let sel = $state<string | null>(null);
  let hoverU = $state<{ u: number; col: number } | null>(null);
  let newer = $state(false);
  let requested = $state<string | null>(null);
  let confirmClear = $state(false);
  const canDesign = $derived(can('conception', 'ecriture'));
  const mine = $derived(!!rack?.lock?.mine);
  // Proportions réelles d'un 19" : façade 482,6 mm pour 44,45 mm par U (≈ 10,86).
  const U = 34;
  const BW = Math.round(U * 10.86) - 22;
  const units = $derived(rack?.container.height_u ?? 0);
  const cols = (w: W) => ({ full: 1, half: 2, third: 3, quarter: 4 })[w];

  async function load() {
    if (!cid) return;
    try { rack = await api.get<Rack>(`/containers/${cid}/rack`); newer = false; } catch (e) { fail(e); }
  }

  // Verrou : prise de main à l'ouverture, battement toutes les 20 s, libération à la sortie
  // ou après 10 minutes sans action.
  let beat: ReturnType<typeof setInterval> | null = null;
  let lastAction = Date.now();
  async function take() {
    if (!cid || !canDesign) return;
    try {
      const r = await api.post<{ mine: boolean; account_name: string }>(`/containers/${cid}/rack/lock`);
      if (r.mine && !beat) beat = setInterval(heartbeat, 20_000);
      await load();
    } catch (e) { fail(e); }
  }
  async function heartbeat() {
    if (!cid) return;
    if (Date.now() - lastAction > 10 * 60_000) { await release(); toast('Main cédée après 10 minutes sans action.', 'info'); return; }
    try { const r = await api.post<{ mine: boolean }>(`/containers/${cid}/rack/lock`); if (!r.mine) { stopBeat(); await load(); } } catch { /* réseau : on réessaie au prochain battement */ }
  }
  function stopBeat() { if (beat) { clearInterval(beat); beat = null; } }
  async function release(id = cid) {
    stopBeat();
    if (!id) return;
    try { await api.del(`/containers/${id}/rack/lock`); } catch { /* déjà libre */ }
    if (id === cid) await load();
  }
  async function requestHand() {
    try { await api.post(`/containers/${cid}/rack/lock/request`); toast(`Demande envoyée à ${rack?.lock?.account_name}.`, 'info'); } catch (e) { fail(e); }
  }

  let current: string | null = null;
  $effect(() => {
    const id = cid;
    if (!id) return;
    untrack(() => {
      if (current && current !== id) release(current);
      current = id; rack = null; sel = null; armed = null; requested = null;
      load().then(take);
    });
  });
  $effect(() => join(`project:${view.project.id}`, (c) => {
    if (c.id !== cid) return;
    if (c.kind === 'rack' && c.action === 'saved' && !mine) newer = true;
    if (c.kind === 'lock' && c.action === 'request' && mine) requested = c.actor;
    // Main prise ailleurs (y compris par soi-même dans un autre onglet) : cet onglet passe en lecture.
    if (c.kind === 'lock' && c.action === 'taken') { if (mine) stopBeat(); load(); }
    if (c.kind === 'lock' && c.action === 'released' && !mine) load();
  }));
  const leave = () => { if (current && beat) fetch(`/api/containers/${current}/rack/lock`, { method: 'DELETE', headers: { 'X-Strata-Request': '1' }, keepalive: true }); };
  onDestroy(() => { stopBeat(); if (current) release(current); });

  // ----- Pose et déplacement -----
  const faceEls = $derived(rack?.elements.filter((e) => e.placement.face === face && !e.placement.shelf_id) ?? []);
  const onShelf = (id: string) => rack?.elements.filter((e) => e.placement.shelf_id === id) ?? [];
  const isShelf = (e: RackEl) => e.accessory?.kind === 'shelf';
  const panelOf = (e: { accessory: Accessory | null }) => (e.accessory?.kind === 'panel' ? e.accessory.panel : null);
  let editPanel = $state<RackEl | null>(null);
  const shelfOf = (e: RackEl) => rack?.elements.find((x) => x.placement.id === e.placement.shelf_id);
  async function putOnShelf(shelf: RackEl, p: Pending) {
    lastAction = Date.now();
    try { await api.post(`/containers/${cid}/placements`, { article_id: p.article_id, instance_id: p.instance_id ?? null, face: shelf.placement.face, u_start: shelf.placement.u_start, shelf_id: shelf.placement.id }); await load(); }
    catch (e) { fail(e); }
  }
  const selected = $derived(rack?.elements.find((e) => e.placement.id === sel) ?? null);
  let placing = false;
  async function placeAt(u: number, col: number) {
    // Une pose à la fois : le clic suivant attend la réponse du serveur.
    if (!armed || !mine || face === 'side' || placing) return;
    lastAction = Date.now();
    const p = armed;
    placing = true;
    try {
      const r = await api.post<{ adjusted: boolean; warning: string | null; placement: Placement }>(`/containers/${cid}/placements`, { article_id: p.article_id, instance_id: p.instance_id ?? null, face, u_start: u, col });
      if (r.adjusted) toast(`Posé en U${r.placement.u_start}, la place libre la plus proche.`, 'info');
      if (r.warning) toast(r.warning, 'warn');
      armed = null; sel = r.placement.id; await load();
      const next = rack?.pending.find((x) => x.article_id === p.article_id && x.rackable);
      if (next) armed = next;
    } catch (e) { fail(e); } finally { placing = false; }
  }
  async function patch(e: RackEl, change: Partial<Placement>) {
    lastAction = Date.now();
    const p = { ...e.placement, ...change };
    try { await api.patch(`/placements/${p.id}`, { rev: e.placement.rev, face: p.face, u_start: p.u_start, width: p.width, col: p.col, circuit_id: p.circuit_id }); await load(); }
    catch (x) { fail(x); await load(); }
  }
  async function unplace(e: RackEl) {
    lastAction = Date.now();
    try { await api.del(`/placements/${e.placement.id}`); sel = null; await load(); } catch (x) { fail(x); }
  }
  async function clearRack() {
    try { const r = await api.post<{ removed: number }>(`/containers/${cid}/placements/clear`); toast(`${r.removed} élément(s) remis à placer.`); confirmClear = false; await load(); } catch (e) { fail(e); }
  }
  function slotFromEvent(ev: MouseEvent, el: HTMLElement): { u: number; col: number } {
    const r = el.getBoundingClientRect();
    const u = Math.max(1, Math.min(units, Math.floor((r.bottom - ev.clientY) / U) + 1));
    const n = armed?.width ? cols(armed.width) : 1;
    const col = Math.min(n - 1, Math.max(0, Math.floor(((ev.clientX - r.left) / r.width) * n)));
    return { u, col };
  }
  // Glisser un élément posé pour le déplacer.
  let dragging = $state<RackEl | null>(null);
  function drop(ev: DragEvent, el: HTMLElement) {
    ev.preventDefault();
    const at = slotFromEvent(ev, el);
    if (dragging) { const d = dragging; dragging = null; patch(d, { u_start: at.u, face: face as Face }); }
    else if (armed) placeAt(at.u, at.col);
  }

  // ----- Circuits -----
  let circ = $state({ name: '', phase: 'L1' as 'L1' | 'L2' | 'L3', rating_a: 16 });
  async function addCircuit(ev: SubmitEvent) {
    ev.preventDefault();
    try { await api.post(`/containers/${cid}/circuits`, { ...circ, name: circ.name || `PDU · ${circ.phase}` }); circ.name = ''; await load(); } catch (e) { fail(e); }
  }
  async function setPduCircuit(v: VPdu, circuit_id: string | null) {
    try { await api.patch(`/articles/${v.article_id}`, { rev: v.rev, quantity: 1, accessory: { ...v.accessory, circuit_id } }); await load(); } catch (e) { fail(e); }
  }
  async function delCircuit(c: Circuit) { try { await api.del(`/circuits/${c.id}`); await load(); } catch (e) { fail(e); } }

  // ----- Rendu -----
  const fnColor = (f: string | null) => (f ? (refById('functions', f)?.data?.color ?? 'var(--fg-3)') : 'var(--fg-3)');
  const CAT_COLORS = ['#2459E0', '#0E8550', '#B78400', '#7A44C9', '#CF3A2F', '#0A8797', '#BD3489', '#D06006', '#69788C'];
  const catColor = (c: string | null) => { if (!c) return '#69788C'; let h = 0; for (const ch of c) h = (h * 31 + ch.charCodeAt(0)) >>> 0; return CAT_COLORS[h % CAT_COLORS.length]!; };
  const legend = $derived([...new Set(rack?.elements.map((e) => e.category ?? 'Sans catégorie') ?? [])]);
  function style(e: RackEl) {
    const n = cols(e.placement.width), l = (100 * e.placement.col) / n, w = 100 / n;
    return `--c:${mode === 'schematic' ? catColor(e.category) : fnColor(e.function)};bottom:calc(${e.placement.u_start - 1} * var(--u) + 1px);height:calc(${e.height_u} * var(--u) - 2px);left:calc(${l}% + 2px);width:calc(${w}% - 4px)` + (mode === 'schematic' ? `;--fp-bg:linear-gradient(180deg,color-mix(in srgb,${catColor(e.category)} 72%,#fff),${catColor(e.category)});--fp-ink:#fff` : '');
  }
  /** Textes : en schématique, les accessoires gardent leur nom (pas de façade dessinée). */
  const rl = (e: RackEl): [string, string] => (mode === 'schematic' && e.nature === 'accessory' ? [e.label, ''] : rackLines(e));
  /** PDU verticaux visibles : seulement en vue arrière. */
  const vpOf = (): VPdu[] => (face !== 'front' && face !== 'side' ? (rack?.vpdus ?? []) : []);
  const typeOf = (e: RackEl) => (e.nature === 'accessory' ? accFace(e as any) : visualOf(e));
  const dw = (e: RackEl) => !!rack?.warnings.some((w) => w.placement_id === e.placement.id);
  // Vue de côté : corps (profondeur) et réserve de câbles, avant depuis la gauche, arrière depuis la droite.
  const depth = $derived(rack?.container.depth_mm ?? 600);
  const SW = 300;
  const sideBars = $derived((rack?.elements ?? []).map((e) => {
    const d = e.depth_mm ?? (e.nature === 'accessory' ? 5 : depth * (e.placement.face === 'front' ? 0.45 : 0.3));
    const res = e.nature === 'accessory' ? 0 : (e.rear_reserve_mm ?? 80);
    const front = e.placement.face === 'front';
    const over = d + res > depth;
    return { e, x: front ? 0 : SW - (d / depth) * SW, w: (d / depth) * SW, rx: front ? (d / depth) * SW : SW - ((d + res) / depth) * SW, rw: (res / depth) * SW, over, front };
  }));
  const wanted = $derived(rack?.pending.filter((p) => p.rackable) ?? []);
  const notRackable = $derived(rack?.pending.filter((p) => !p.rackable) ?? []);
  // À placer, rangé par catégorie : équipements par fonction (Vidéo, Audio…), puis panels et
  // accessoires ; dans chaque catégorie, par nom.
  const catOf = (p: Pending): { key: string; label: string; color: string; order: number } => {
    if (p.accessory?.kind === 'panel') return { key: '~panel', label: 'Panels de brassage', color: 'var(--fg-3)', order: 1000 };
    if (p.nature === 'accessory') return { key: '~acc', label: 'Accessoires', color: 'var(--fg-3)', order: 1001 };
    // Fonction de la fiche, sinon celle de son type (ou du type parent).
    const t = p.type_id ? refById('types', p.type_id) : null;
    const fk = p.function ?? t?.data?.function ?? (t?.parent_id ? refById('types', t.parent_id)?.data?.function : null) ?? null;
    const f = fk ? refById('functions', fk) : null;
    return f ? { key: f.id, label: f.name, color: f.data?.color ?? 'var(--fg-3)', order: f.position ?? 500 } : { key: '~none', label: 'Autres équipements', color: 'var(--fg-3)', order: 999 };
  };
  const wantedGroups = $derived.by(() => {
    const m = new Map<string, { key: string; label: string; color: string; order: number; list: Pending[] }>();
    for (const p of wanted) { const c = catOf(p); if (!m.has(c.key)) m.set(c.key, { ...c, list: [] }); m.get(c.key)!.list.push(p); }
    for (const g of m.values()) g.list.sort((a, b) => a.label.localeCompare(b.label, 'fr') || (a.tag ?? '').localeCompare(b.tag ?? ''));
    return [...m.values()].sort((a, b) => a.order - b.order || a.label.localeCompare(b.label, 'fr'));
  });
  const FACES: Record<Face | 'side', string> = { front: 'Avant', rear: 'Arrière', rear_inner: 'Arrière intérieure', side: 'Côté' };
</script>

<svelte:window onbeforeunload={leave} onkeydown={(e) => { if (e.key === 'Escape') { armed = null; sel = null; } }} />

{#if all && racks.length}
  <AllRacks {view} loc={route.path.split('/')[5] ?? null} />
{:else if !racks.length && !fixed}
  <div class="empty">Aucun élément rackable dans ce projet. Ajoutez une baie ou un flight-case depuis l’<a class="ilink" href="/projets/{view.project.id}/inventaire">inventaire</a>.</div>
{:else if rack}
  {@const t = rack.totals}
  <div class="rack-screen card" style="padding:0;overflow:hidden">
    {#if !mine || requested}<div class="lockbar" class:mine class:other={!mine}>
      {#if mine}
        <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{requested}</b> demande la main.</span></span>
        <span class="sp"><button class="btn sm" onclick={() => { requested = null; release(); }}>Céder la main</button></span>
      {:else if rack.lock}
        <span class="lk"><span class="key-i"><Icon name="lock" /></span><span><b>{rack.lock.account_name}</b> a la main depuis {relative(rack.lock.taken_at)}. Vous voyez la dernière version enregistrée.</span></span>
        <span class="sp">{#if newer}<button class="btn sm" onclick={load}><Icon name="refresh" size="sm" />Nouvelle version : recharger</button>{/if}{#if canDesign}<button class="btn sm" onclick={requestHand}>Demander la main</button>{/if}</span>
      {:else}
        <span class="lk"><span class="key-i"><Icon name="unlock" /></span><span>Personne ne modifie cette élévation.</span></span>
        <span class="sp">{#if canDesign}<button class="btn sm primary" onclick={take}>Prendre la main</button>{:else}<span class="muted small">Lecture seule : votre rôle ne permet pas de concevoir.</span>{/if}</span>
      {/if}
    </div>{/if}
    <div class="dtool">
      {#if !fixed}<RackPicker {view} value={cid ?? ''} onpick={(v) => navigate(`/projets/${view.project.id}/racks/${v.replace(':', '/')}`)} />{/if}
      <div class="seg" role="group" aria-label="Face">{#each [...rack.faces, 'side'] as f}<button class:on={face === f} onclick={() => { face = f as Face | 'side'; armed = null; }}>{FACES[f as Face]}</button>{/each}</div>
      <div class="seg" role="group" aria-label="Rendu"><button class:on={mode === 'real' && !show3d} onclick={() => { mode = 'real'; show3d = false; }}>Réel</button><button class:on={mode === 'schematic' && !show3d} onclick={() => { mode = 'schematic'; show3d = false; }}>Schématique</button><button class:on={show3d} onclick={() => (show3d = true)}><Icon name="box" size="sm" />3D</button></div>
      {#if !show3d}<label class="rtx" title="Textes affichés sur les équipements"><span>Textes</span><select class="input" value={rackText.v} onchange={(e) => setRackText((e.target as HTMLSelectElement).value as RackText)}>{#each Object.entries(RACK_TEXT) as [k, l]}<option value={k}>{l}</option>{/each}</select></label>{/if}
      <span class="gap"></span>
      <span class="meter" class:err={t.over_u}><span>U</span><Vu value={(t.used_u / (t.total_u || 1)) * 100} mode="load" n={16} /><span class="mono">{nf(t.used_u, t.used_u % 1 ? 1 : 0)} / {t.total_u} U</span></span>
      <span class="meter" class:err={t.over_weight}><span>Poids</span><Vu value={rack.container.max_weight_kg ? (t.weight_kg / rack.container.max_weight_kg) * 100 : 0} mode="load" n={16} /><span class="mono">{nf(t.weight_kg, 1)}{rack.container.max_weight_kg ? ` / ${nf(rack.container.max_weight_kg)}` : ''} kg</span></span>
      <span class="meter" class:err={t.over_power}><span>Puiss.</span><Vu value={rack.container.max_power_w ? (t.power_w / rack.container.max_power_w) * 100 : 0} mode="load" n={16} /><span class="mono">{nf(t.power_w)}{rack.container.max_power_w ? ` / ${nf(rack.container.max_power_w)}` : ''} W</span></span>
    </div>

    <div class="design-body" class:ro={!mine}>
      <aside class="pane left">
        <section>
          <h2>À placer <span class="muted">· {wanted.length}</span></h2>
          {#if mine}<p class="small muted">Choisissez un élément, puis une position dans la baie. Échap pour annuler.</p>{/if}
          {#each wantedGroups as g (g.key)}
            <div class="pcat"><i style="--c:{g.color}"></i><span>{g.label}</span><b>{g.list.length}</b></div>
          {#each g.list as p (p.instance_id ?? p.article_id)}
            <button class="pend mini" class:on={armed === p} disabled={!mine || face === 'side'} onclick={() => (armed = armed === p ? null : p)} title="{p.label}{p.tag ? ` · ${p.tag}` : ''}">
              <i class="pm-c" style="--c:{fnColor(p.function)}"></i><span class="pm-l">{p.label}</span>{#if p.tag}<span class="pm-t">{p.tag}</span>{/if}<span class="pm-u">{p.height_u} U{p.width && p.width !== 'full' ? ` · ${p.width === 'half' ? '½' : p.width === 'third' ? '⅓' : '¼'}` : ''}</span>
            </button>
          {/each}
          {:else}<p class="small muted">Tout est posé.</p>{/each}
          {#if notRackable.length}
            <h2 style="margin-top:8px">Sur étagère <span class="muted">· {notRackable.length}</span></h2>
            <p class="small muted">Hors format rack : choisissez une étagère posée dans la baie, puis « Poser ».</p>
            {#each notRackable as p (p.instance_id ?? p.article_id)}<div class="pend mini" style="cursor:default"><i class="pm-c" style="--c:{fnColor(p.function)}"></i><span class="pm-l">{p.label}</span>{#if p.tag}<span class="pm-t">{p.tag}</span>{/if}</div>{/each}
          {/if}
        </section>
      </aside>

      <div class="rack-stage" class:stage3d={show3d}>
        {#if show3d}
          <Rack3D container={rack.container} elements={rack.elements} vpdus={rack.vpdus ?? []} title={rack.container.name} />
        {:else if face === 'side'}
          <div class="rack-wrap">
            <span class="rack-title"><Icon name="server" />{rack.container.name} · côté</span>
            <div class="rack" style="--units:{units};--u:{U}px;grid-template-columns:28px auto">
              <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
              <div>
                <div class="side-body" style="--w:{SW}px">
                  {#each sideBars as b (b.e.placement.id)}
                    <span class="sbar" style="--c:{fnColor(b.e.function)};bottom:calc({b.e.placement.u_start - 1} * var(--u) + 1px);height:calc({b.e.height_u} * var(--u) - 3px);left:{b.x}px;width:{b.w}px" title="{b.e.label} · {b.e.depth_mm ?? '?'} mm"></span>
                    {#if b.rw > 0}<span class="sres" class:over={b.over} style="bottom:calc({b.e.placement.u_start - 1} * var(--u) + 1px);height:calc({b.e.height_u} * var(--u) - 3px);left:{Math.min(b.rx, b.rx + b.rw)}px;width:{Math.abs(b.rw)}px"></span>{/if}
                  {/each}
                </div>
                <div class="side-legend"><span>Avant</span><span>{depth} mm</span><span>Arrière</span></div>
              </div>
            </div>
          </div>
        {:else}
          <div class="rack-wrap">
            <span class="rack-title"><Icon name="server" />{rack.container.name} · {FACES[face].toLowerCase()}</span>
            <div class="rack" class:placing={!!armed} class:vpl={vpOf().some((v) => v.accessory.side === 'left')} class:vpr={vpOf().some((v) => v.accessory.side === 'right')} style="--units:{units};--u:{U}px;--bw:{BW}px">
              <!-- PDU verticaux : couloirs arrière, gauche et droite vus de l'arrière. -->
              {#each vpOf() as v, i (v.article_id)}
                {@const h = Math.min(units * U, (v.accessory.length_mm / 44.45) * U)}
                {@const left = v.accessory.side === 'left'}
                {@const k = vpOf().filter((x, j) => j < i && x.accessory.side === v.accessory.side).length}
                <div class="vpdu" style="{left ? 'left' : 'right'}:{6 + k * 20}px;bottom:calc(24px + {(units * U - h) / 2}px);height:{h}px;--n:{v.accessory.outlets}" title={v.label}><i></i></div>
              {/each}
              <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
              <div class="rack-body" role="grid" tabindex="-1" aria-label="Élévation"
                onmousemove={(e) => { if (armed) hoverU = slotFromEvent(e, e.currentTarget as HTMLElement); }}
                onmouseleave={() => (hoverU = null)}
                onclick={(e) => { if (armed && !(e.target as Element).closest('.ritem')) { const at = slotFromEvent(e, e.currentTarget as HTMLElement); placeAt(at.u, at.col); } else if (!(e.target as Element).closest('.ritem')) sel = null; }}
                onkeydown={(e) => { if (e.key === 'Escape') { armed = null; sel = null; } }}
                ondragover={(e) => { if (mine) e.preventDefault(); }}
                ondrop={(e) => drop(e, e.currentTarget as HTMLElement)}>
                {#each faceEls as e (e.placement.id)}
                  <div class="ritem {e.nature === 'accessory' ? 'acc' : ''} {ALU.has(typeOf(e)) && mode === 'real' ? 'alu' : ''} {e.height_u <= 1 ? 'u1' : ''} {e.placement.width !== 'full' ? `half c${e.placement.col}` : ''}" class:sel={sel === e.placement.id} class:dw={dw(e)}
                    style={style(e)} role="button" tabindex="0" draggable={mine} title="{e.custom ? `${e.custom} · ` : ''}{e.tag ? `${e.tag} · ` : ''}{e.label} · U{e.placement.u_start}"
                    onclick={() => (sel = e.placement.id)} onkeydown={(k) => { if (k.key === 'Enter') sel = e.placement.id; }}
                    ondragstart={() => (dragging = e)} ondragend={() => (dragging = null)}>
                    {#if e.nature !== 'accessory'}<i class="led"></i>{/if}<span class="rl" class:two={!!rl(e)[1]}>{#if rl(e)[0]}<b class="rn" class:id={!!e.tag && rl(e)[0] === e.tag}>{rl(e)[0]}</b>{/if}{#if rl(e)[1]}<small>{rl(e)[1]}</small>{/if}</span>{#if isShelf(e) && onShelf(e.placement.id).length}<span class="shelf-load">{#each onShelf(e.placement.id) as x (x.placement.id)}<span class="sh-it" style="--c:{fnColor(x.function)}">{x.label}</span>{/each}</span>{:else if mode === 'real' && panelOf(e)}<span class="pdec"><PanelFace panel={panelOf(e)!} /></span>{:else if mode === 'real'}<span class="dec">{@html decor(typeOf(e))}</span>{:else}<span class="dec"></span>{/if}
                  </div>
                {/each}
                {#if armed && hoverU}
                  {@const n = armed.width ? cols(armed.width) : 1}
                  <div class="rghost" style="bottom:calc({Math.min(hoverU.u, units - Math.ceil(armed.height_u ?? 1) + 1) - 1} * var(--u));height:calc({armed.height_u ?? 1} * var(--u));left:calc({(100 * hoverU.col) / n}% + 2px);width:calc({100 / n}% - 4px)"></div>
                {/if}
              </div>
              <div class="rack-scale">{#each Array(units) as _, i}<span>{units - i}</span>{/each}</div>
            </div>
          </div>
        {/if}
      </div>

      <aside class="pane right">
        {#if selected}
          {@const p = selected.placement}
          <section>
            <h2 class="sel-h"><span>{selected.label}</span><button class="icon-btn sm" title="Désélectionner (Échap)" onclick={() => (sel = null)}><Icon name="x" /></button></h2>
            <dl class="kv"><dt>Repère</dt><dd class="mono">{selected.tag ?? '—'}</dd><dt>Position</dt><dd>U{p.u_start}{selected.height_u > 1 ? `–${p.u_start + Math.ceil(selected.height_u) - 1}` : ''} · {FACES[p.face]}</dd>
              <dt>Profondeur</dt><dd>{selected.depth_mm ? `${nf(selected.depth_mm)} mm` : '—'}</dd><dt>Puissance</dt><dd>{selected.power_w ? `${nf(selected.power_w)} W` : '—'}</dd></dl>
            {#each rack.warnings.filter((w) => w.placement_id === p.id) as w}<div class="alert warn"><Icon name="alert" /><span>{w.message}</span></div>{/each}
            {#if panelOf(selected)}
              {@const pn = panelOf(selected)!}
              <div class="pside"><span class="pface-s" style="--h:{pn.height_u}"><PanelFace panel={pn} /></span><span class="muted small">{pn.slots.length} connecteur{pn.slots.length > 1 ? 's' : ''} sur {pn.height_u * pn.per_u} · {pn.groups.length} groupe{pn.groups.length > 1 ? 's' : ''}</span>
                <button class="btn sm" onclick={() => (editPanel = selected)}><Icon name="grid" size="sm" />{mine ? 'Modifier le panel' : 'Voir le panel'}</button></div>
            {/if}
            {#if isShelf(selected)}
              <div class="stack" style="gap:8px">
                <span class="flabel">Sur l’étagère · {onShelf(p.id).length} / 10</span>
                {#each onShelf(p.id) as x (x.placement.id)}<button class="pend" style="padding:8px 10px" onclick={() => (sel = x.placement.id)}><span class="pend-t"><b>{x.label}</b><span>{x.tag ?? ''}</span></span></button>{/each}
                {#if mine && notRackable.length && onShelf(p.id).length < 10}
                  {#each notRackable as np (np.instance_id ?? np.article_id)}<button class="btn sm" style="justify-content:flex-start" onclick={() => putOnShelf(selected, np)}><Icon name="plus" size="sm" />Poser {np.label}{np.tag ? ` · ${np.tag}` : ''}</button>{/each}
                {/if}
              </div>
            {/if}
            {#if selected.placement.shelf_id}
              <p class="small muted">Posé sur {shelfOf(selected)?.label ?? 'une étagère'} : il en suit la position.</p>
              {#if mine}
                <div class="field"><label for="rk-c2">Circuit</label><select class="input" id="rk-c2" value={p.circuit_id ?? ''} onchange={(e) => patch(selected, { circuit_id: (e.target as HTMLSelectElement).value || null })}><option value="">Non branché</option>{#each rack.circuits as c}<option value={c.id}>{c.name} · {c.phase} · {c.rating_a} A</option>{/each}</select></div>
                <button class="btn sm ghost danger" style="align-self:flex-start" onclick={() => unplace(selected)}><Icon name="x" size="sm" />Retirer de l’étagère</button>
              {/if}
            {:else if mine}
              <div style="display:flex;gap:6px;flex-wrap:wrap">
                <button class="btn sm" onclick={() => patch(selected, { u_start: p.u_start + 1 })} disabled={p.u_start + selected.height_u > units}>▲ Monter</button>
                <button class="btn sm" onclick={() => patch(selected, { u_start: p.u_start - 1 })} disabled={p.u_start <= 1}>▼ Descendre</button>
              </div>
              <div class="field"><label for="rk-face">Face</label><select class="input" id="rk-face" value={p.face} onchange={(e) => patch(selected, { face: (e.target as HTMLSelectElement).value as Face })}>{#each rack.faces as f}<option value={f}>{FACES[f]}</option>{/each}</select></div>
              {#if selected.nature !== 'accessory'}
                <div class="field"><label for="rk-w">Largeur et colonne</label><select class="input" id="rk-w" value="{p.width}:{p.col}" onchange={(e) => { const [w, c] = (e.target as HTMLSelectElement).value.split(':'); patch(selected, { width: w as W, col: Number(c) }); }}>
                  <option value="full:0">Pleine largeur</option>
                  {#each [['half', 2, '1/2'], ['third', 3, '1/3'], ['quarter', 4, '1/4']] as [w, n, l]}{#each Array(n) as _, c}<option value="{w}:{c}">{l} · colonne {c + 1}</option>{/each}{/each}
                </select></div>
                <div class="field"><label for="rk-c">Circuit</label><select class="input" id="rk-c" value={p.circuit_id ?? ''} onchange={(e) => patch(selected, { circuit_id: (e.target as HTMLSelectElement).value || null })}><option value="">Non branché</option>{#each rack.circuits as c}<option value={c.id}>{c.name} · {c.phase} · {c.rating_a} A</option>{/each}</select></div>
              {/if}
              <button class="btn sm ghost danger" style="align-self:flex-start" onclick={() => unplace(selected)}><Icon name="x" size="sm" />Retirer de la baie</button>
            {/if}
          </section>
        {:else}
        <section>
          <h2>Énergie par phase</h2>
          {#each rack.electrical.phases as ph}
            {@const rating = rack.electrical.circuits.filter((c) => c.phase === ph.phase).reduce((s, c) => s + c.rating_a, 0)}
            <div class="meter" style="grid-template-columns:28px minmax(0,1fr) auto"><span class="mono">{ph.phase}</span><Vu value={rating ? (ph.amps / rating) * 100 : 0} mode="load" n={20} /><span class="mono">{nf(ph.amps, 1)}{rating ? ` / ${rating}` : ''} A</span></div>
          {/each}
          {#if rack.electrical.unbalanced}<div class="alert warn"><Icon name="alert" /><span>Phases déséquilibrées : écart de {rack.electrical.imbalance_pct} %.</span></div>{/if}
          {#if rack.electrical.unassigned_watts}<p class="small muted">{nf(rack.electrical.unassigned_watts)} W posés sans circuit.</p>{/if}
          {#each rack.electrical.circuits as c}
            <div class="circ" class:over={c.over}><span><b>{c.name}</b><span class="small muted"> · {c.phase} · {c.rating_a} A</span></span><span class="mono">{nf(c.amps, 1)} A</span>{#if mine}<button class="icon-btn sm" title="Supprimer le circuit" onclick={() => delCircuit(rack!.circuits.find((x) => x.id === c.id)!)}><Icon name="x" /></button>{/if}</div>
          {/each}
          {#if mine}
            <form class="circ-form" onsubmit={addCircuit}>
              <input class="input" bind:value={circ.name} placeholder="PDU A · prise 1" aria-label="Nom du circuit">
              <select class="input" bind:value={circ.phase} aria-label="Phase"><option>L1</option><option>L2</option><option>L3</option></select>
              <select class="input" bind:value={circ.rating_a} aria-label="Calibre"><option value={16}>16 A</option><option value={32}>32 A</option></select>
              <button class="btn sm"><Icon name="plus" size="sm" />Circuit</button>
            </form>
          {/if}
        </section>
        {#if rack.vpdus?.length}
          <section>
            <h2>PDU verticaux</h2>
            {#each rack.vpdus as v (v.article_id)}
              <div class="vp-row" class:over={v.over}>
                <span class="vp-n"><b>{v.accessory.side === 'left' ? 'Gauche' : 'Droite'} · {v.accessory.outlets} prises</b><small>{v.accessory.circuit_id ? `${v.fed} occupée${v.fed > 1 ? 's' : ''} · ${v.over ? `${v.fed - v.accessory.outlets} de trop` : `${v.free} libre${v.free > 1 ? 's' : ''}`}` : 'Rattachez un circuit pour compter les prises'}</small></span>
                <select class="input" aria-label="Circuit d’alimentation" disabled={!mine || !rack.circuits.length} value={v.accessory.circuit_id ?? ''} onchange={(e) => setPduCircuit(v, (e.target as HTMLSelectElement).value || null)}><option value="">{rack.circuits.length ? 'Sans circuit' : 'Aucun circuit'}</option>{#each rack.circuits as c}<option value={c.id}>{c.name} · {c.phase}</option>{/each}</select>
              </div>
            {/each}
            {#if rack.vpdus.some((v) => v.over)}<div class="alert warn"><Icon name="alert" /><span>Plus d’équipements alimentés que de prises sur un PDU : ajoutez un PDU ou répartissez les équipements sur un autre circuit.</span></div>{/if}
            <p class="small muted">Prises occupées : équipements posés, avec une consommation, rattachés au même circuit que le PDU.</p>
          </section>
        {/if}
        <section>
          <h2>Totaux</h2>
          <dl class="kv"><dt>Hauteur</dt><dd>{nf(t.used_u, t.used_u % 1 ? 1 : 0)} / {t.total_u} U</dd><dt>Poids</dt><dd>{nf(t.weight_kg, 1)} kg</dd><dt>Puissance</dt><dd>{nf(t.power_w)} W</dd><dt>Dissipation</dt><dd>{nf(t.power_w * 3.412)} BTU/h</dd></dl>
          {#each rack.warnings as w}<div class="alert warn"><Icon name="alert" /><span>{w.message}</span></div>{/each}
          {#if mode === 'schematic'}<div class="legend" style="grid-template-columns:1fr">{#each legend as l}<div><span class="sw" style="--c:{catColor(l === 'Sans catégorie' ? null : l)}"></span>{l}</div>{/each}</div>{/if}
          {#if mine && rack.elements.length}<button class="btn sm ghost danger" style="align-self:flex-start" onclick={() => (confirmClear = true)}>Vider le rack</button>{/if}
        </section>
        {/if}
      </aside>
    </div>
  </div>
{:else}<span class="spin"></span>{/if}

{#if confirmClear}
  <Confirm title="Vider le rack ?" eyebrow={rack?.container.name} confirmLabel="Vider" danger onconfirm={clearRack} onclose={() => (confirmClear = false)}>
    <p>Tous les éléments redeviennent « à placer ». L’inventaire ne change pas.</p>
  </Confirm>
{/if}

{#if editPanel}
  <ProjectPanel article={{ id: editPanel.placement.article_id, rev: editPanel.article_rev, accessory: editPanel.accessory, label: editPanel.label } as Article} readonly={!mine} eyebrow="Panel du rack" onclose={() => (editPanel = null)} ondone={load} />
{/if}

<style>
  .rack-screen :global(.rack-body) { width: var(--bw, 370px); }
  .sel-h { justify-content: space-between; align-items: center; }
  .sel-h span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pcat { display: flex; align-items: center; gap: 8px; margin: 10px 0 2px; padding: 0 2px; font: 700 11.5px var(--font); letter-spacing: .07em; text-transform: uppercase; color: var(--fg); }
  .pcat i { width: 8px; height: 8px; border-radius: 50%; background: var(--c); }
  .pcat b { margin-left: auto; font: 600 11px var(--mono); padding: 1px 7px; border-radius: 99px; background: var(--surface-3); color: var(--fg-2); }
  .pend.mini { flex-direction: row; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 8px; min-height: 34px; }
  .pm-c { flex: none; width: 4px; align-self: stretch; border-radius: 2px; background: var(--c); }
  .pm-l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13.5px !important; font-weight: 600; color: var(--fg) !important; }
  .pm-t { flex: none; font: 600 11px/1 var(--mono) !important; color: var(--fg-3); }
  .pm-u { flex: none; font: 600 11px/1 var(--mono) !important; padding: 3px 6px; border-radius: 5px; background: var(--sunken); color: var(--fg-2) !important; }
  .rack-stage.stage3d { padding: 0; display: block; align-self: stretch; }
  /* Hauteur fixe : la baie défile seule, les panneaux latéraux restent visibles. */
  .rack-screen .design-body { height: calc(100dvh - 236px); min-height: 560px; }
  .shelf-load { flex: 1 1 0; min-width: 0; display: flex; gap: 4px; justify-content: flex-end; overflow: hidden; }
  .sh-it { flex: none; font: 600 10px/1 var(--font); padding: 3px 6px; border-radius: 3px; color: #fff; background: color-mix(in srgb, var(--c) 70%, #1C1F23); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, .15); max-width: 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .circ { display: grid; grid-template-columns: minmax(0, 1fr) auto auto; gap: 8px; align-items: center; padding: 8px 10px; border-radius: 10px; background: var(--surface-2); font-size: 14px; }
  .circ.over { background: var(--err-soft); }
  .circ.over .mono { color: var(--err); font-weight: 700; }
  .circ-form { display: grid; grid-template-columns: minmax(0, 1fr) 70px 80px auto; gap: 6px; }
  .circ-form .input { height: 36px; font-size: 14px; padding: 0 8px; }
  .pdec { flex: 1 1 0; min-width: 0; align-self: stretch; display: block; padding: 2px 0; margin-left: 8px; }
  .pside { display: grid; gap: 8px; justify-items: start; }
  .pface-s { display: block; width: 100%; height: calc(var(--h) * 26px); background: linear-gradient(180deg, #363B42, #1D2025); border-radius: 4px; padding: 2px 4px; }
  @media (max-width: 1100px) { .rack-screen .design-body { grid-template-columns: minmax(0, 1fr); height: auto; } }
  .rtx { display: inline-flex; align-items: center; gap: 6px; font: 600 10.5px var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--fg-3); }
  .rtx .input { height: 30px; width: auto; padding: 0 26px 0 9px; font: 500 13px var(--font); letter-spacing: 0; text-transform: none; }
  .vp-row { display: grid; grid-template-columns: minmax(0, 1fr) 150px; gap: 8px; align-items: center; padding: 6px 0; border-bottom: 1px solid var(--line); }
  .vp-row .input { height: 32px; font-size: 13px; }
  .vp-n { display: flex; flex-direction: column; line-height: 1.25; min-width: 0; }
  .vp-n small { color: var(--fg-3); font-size: 12px; }
  .vp-row.over .vp-n small { color: var(--err); }
  /* PDU vertical : bandeau étroit, disjoncteur en tête, prises réparties sur la longueur. */
  .rack.vpl { padding-left: 26px; }
  .rack.vpr { padding-right: 26px; }
  .vpdu { position: absolute; z-index: 2; width: 16px; border-radius: 3px; background: #444A52; box-shadow: inset 0 0 0 1px #000, 0 0 0 1px rgba(255, 255, 255, .08); }
  .vpdu::before { content: ""; position: absolute; left: 4px; right: 4px; top: 5px; height: 9px; border-radius: 1px; background: linear-gradient(180deg, #A9B0B8 0 45%, #2A2E33 45%); }
  .vpdu i { position: absolute; left: 3px; right: 3px; top: 20px; bottom: 6px; border-radius: 1px; background: repeating-linear-gradient(180deg, #08090A 0 8px, transparent 8px 12px); }
</style>
