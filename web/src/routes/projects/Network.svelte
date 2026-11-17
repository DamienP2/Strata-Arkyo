<script lang="ts">
  import { ask } from '../../lib/ask.svelte';
  // Réseau (section 11.3) : VLAN, ports réseau (accès ou trunk, IP, nom d'hôte, réseau A/B),
  // plan multicast SMPTE ST 2110 et carte du réseau. Écran partagé en direct.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import EmptyState from '../../components/EmptyState.svelte';
  import Loading from '../../components/Loading.svelte';
  import Seg from '../../components/Seg.svelte';
  import SaveState from '../../components/SaveState.svelte';
  import FieldError from '../../components/FieldError.svelte';
  import SortTh, { type Sort } from '../../components/SortTh.svelte';
  import { route } from '../../lib/router.svelte';
  import { familyColor } from '../../lib/catalog.svelte';
  import { api, download, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import type { ProjectView } from '../../lib/projects';
  import { pl } from '../../lib/format';

  interface Vlan { id: string; vid: number; name: string; color: string; cidr: string | null; gateway: string | null; role: string | null; ports: number; hosts: number | null; used: number | null }
  interface NetPort { ref: string; node: string; equipment: string; tag: string; element: string; port: string; connector: string; switching: boolean; mode: 'access' | 'trunk'; vlan: number | null; vlans: number[]; ip: string | null; hostname: string | null; redundancy: string | null; protocol: string | null; peer: string | null; peer_ref: string | null }
  interface Flow { key: string; instance: string; equipment: string; tag: string; element: string; signal: string; essence: 'video' | 'audio' | 'data'; n: number; address: string | null; udp: number | null; manual: boolean }
  interface Issue { kind: string; message: string; refs: string[] }
  interface MapNode { key: string; label: string; tag: string; element: string; switch: boolean; ports: number; vlans: number[]; ips: string[] }
  interface MapLink { a: string; b: string; a_port: string; b_port: string; redundancy: string | null; trunk: boolean; vlans: number[] }
  interface R { vlans: Vlan[]; ports: NetPort[]; flows: Flow[]; issues: Issue[]; map: { nodes: MapNode[]; links: MapLink[] }; stats: { ports: number; with_ip: number; flows: number; flows_assigned: number; issues: number } }

  let { view }: { view: ProjectView } = $props();
  const pid = $derived(view.project.id);
  const writable = $derived(can('conception', 'ecriture'));
  const ESS: Record<Flow['essence'], string> = { video: 'Vidéo', audio: 'Audio', data: 'Données' };
  /** Essence d'un flux → famille de signal (Vidéo, Audio, Data) : même couleur que dans le catalogue. */
  const ESS_FAMILY: Record<Flow['essence'], string> = { video: 'V', audio: 'A', data: 'N' };
  const PROTO: [string, string][] = [['st2110-20', 'ST 2110-20'], ['st2110-30', 'ST 2110-30'], ['st2110-40', 'ST 2110-40'], ['dante', 'Dante'], ['ndi', 'NDI'], ['madi-ip', 'MADI-IP'], ['generic', 'Générique']];
  let mapVlan = $state<number | ''>('');

  let r = $state<R | null>(null);
  // Onglet dans l'adresse (?onglet=vlan|multicast|carte), pour y revenir et le partager.
  type Tab = 'ports' | 'vlans' | 'flows' | 'map';
  const SLUG: Record<Tab, string> = { ports: 'ports', vlans: 'vlan', flows: 'multicast', map: 'carte' };
  const tab = $derived<Tab>((Object.keys(SLUG) as Tab[]).find((k) => SLUG[k] === route.query.get('onglet')) ?? 'ports');
  function setTab(k: Tab) {
    const u = new URL(location.href);
    if (k === 'ports') u.searchParams.delete('onglet'); else u.searchParams.set('onglet', SLUG[k]);
    // Changer d'onglet ne crée pas d'entrée d'historique et ne fait pas défiler la page.
    history.replaceState(history.state, '', u.pathname + u.search);
    route.query = new URLSearchParams(u.search);
  }
  let q = $state('');
  let vf = $state<'' | 'none' | number>('');
  /** Ports choisis (cases de la première colonne). */
  let sel = $state(new Set<string>());
  let busy = $state(false);

  async function load() { try { r = await api.get(`/projects/${pid}/network`); } catch (e) { fail(e); } }
  $effect(() => { view; untrack(load); });

  const vlanOf = (vid: number | null) => r?.vlans.find((v) => v.vid === vid);
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const issueRefs = $derived(new Set(r?.issues.flatMap((i) => i.refs) ?? []));

  // ----- Tri des ports (mémorisé dans ce navigateur) -----
  const SORT_KEY = 'strata.network.ports.sort';
  function readSort(): Sort {
    try { const x = JSON.parse(localStorage.getItem(SORT_KEY) ?? 'null'); if (x && typeof x.key === 'string' && (x.dir === 1 || x.dir === -1)) return x; } catch { /* stockage indisponible */ }
    return { key: '', dir: 1 };
  }
  let sort = $state<Sort>(readSort());
  $effect(() => { const x = JSON.stringify({ key: sort.key, dir: sort.dir }); try { localStorage.setItem(SORT_KEY, x); } catch { /* stockage indisponible */ } });
  const coll = new Intl.Collator('fr', { numeric: true, sensitivity: 'base' });
  const ipNum = (ip: string | null) => { const m = ip?.match(/^(\d+)\.(\d+)\.(\d+)\.(\d+)/); return m ? ((Number(m[1]) * 256 + Number(m[2])) * 256 + Number(m[3])) * 256 + Number(m[4]) : null; };
  function sortVal(p: NetPort): string | number | null {
    switch (sort.key) {
      case 'equipment': return `${p.equipment} ${p.tag} ${p.port}`;
      case 'vlan': return p.vlan;
      case 'ip': return ipNum(p.ip) ?? (p.ip || null);
      default: return null;
    }
  }
  /** Valeurs vides toujours en fin de liste, quel que soit le sens. */
  function cmp(a: NetPort, b: NetPort): number {
    const x = sortVal(a), y = sortVal(b);
    const ex = x == null || x === '', ey = y == null || y === '';
    if (ex || ey) return ex && ey ? 0 : ex ? 1 : -1;
    return sort.dir * (typeof x === 'number' && typeof y === 'number' ? x - y : coll.compare(String(x), String(y)));
  }
  const shown = $derived.by(() => {
    const list = (r?.ports ?? []).filter((p) =>
      (!q || norm(`${p.element} ${p.equipment} ${p.tag} ${p.port} ${p.ip ?? ''} ${p.hostname ?? ''}`).includes(norm(q)))
      && (vf === '' || (vf === 'none' ? p.vlan == null && !p.vlans.length : p.vlan === vf || p.vlans.includes(vf as number))));
    return sort.key ? list.sort(cmp) : list;
  });
  const allSel = $derived(shown.length > 0 && shown.every((p) => sel.has(p.ref)));
  function selAll() { const s = new Set(sel); if (allSel) shown.forEach((p) => s.delete(p.ref)); else shown.forEach((p) => s.add(p.ref)); sel = s; }
  function toggleSel(ref: string) { const s = new Set(sel); if (s.has(ref)) s.delete(ref); else s.add(ref); sel = s; }

  async function setPorts(refs: string[], set: Record<string, unknown>) {
    if (!refs.length) return;
    busy = true;
    try { await api.post(`/projects/${pid}/network/ports`, { refs, set }); await load(); }
    catch (e) { fail(e); await load(); } finally { busy = false; }
  }
  async function autoIp(refs?: string[]) {
    busy = true;
    try {
      const x = await api.post<{ assigned: number; skipped: number; full: number }>(`/projects/${pid}/network/ip/auto`, refs ? { refs } : {});
      const bits = [`${x.assigned} adresse${x.assigned > 1 ? 's' : ''} attribuée${x.assigned > 1 ? 's' : ''}`];
      if (x.skipped) bits.push(`${x.skipped} port${x.skipped > 1 ? 's' : ''} sans VLAN à sous-réseau`);
      if (x.full) bits.push(`${x.full} sans place libre`);
      toast(bits.join(' · ') + '.', x.skipped || x.full ? 'warn' : undefined);
      await load();
    } catch (e) { fail(e); } finally { busy = false; }
  }
  /** Ports choisis encore présents (un port peut disparaître pendant un rechargement). */
  const selRefs = $derived([...sel].filter((x) => r?.ports.some((p) => p.ref === x)));

  // ----- VLAN -----
  let edit = $state<{ id: string | null; vid: number | null; name: string; color: string; cidr: string; gateway: string; role: string } | null>(null);
  let errs = $state<ApiError | null>(null);
  const PALETTE = ['#3A7BD5', '#0E8550', '#E0A030', '#CF3A2F', '#8A4FD8', '#1AA3A3', '#D9468F', '#5B636D'];
  function newVlan() {
    const used = new Set(r?.vlans.map((v) => v.vid));
    let vid = 10; while (used.has(vid)) vid += 10;
    edit = { id: null, vid, name: '', color: PALETTE[(r?.vlans.length ?? 0) % PALETTE.length]!, cidr: '', gateway: '', role: '' }; errs = null;
  }
  function editVlan(v: Vlan) { edit = { id: v.id, vid: v.vid, name: v.name, color: v.color, cidr: v.cidr ?? '', gateway: v.gateway ?? '', role: v.role ?? '' }; errs = null; }
  async function saveVlan() {
    if (!edit) return;
    const body = { ...edit, vid: Number(edit.vid), cidr: edit.cidr || null, gateway: edit.gateway || null, role: edit.role || null };
    try {
      if (edit.id) await api.patch(`/projects/${pid}/network/vlans`, body); else await api.post(`/projects/${pid}/network/vlans`, body);
      toast(edit.id ? `VLAN ${body.vid} enregistré.` : `VLAN ${body.vid} créé.`); edit = null; await load();
    } catch (e) { if (e instanceof ApiError && e.fields.length) errs = e; else fail(e); }
  }
  async function delVlan(v: Vlan) {
    const ok = await ask({ title: `Supprimer le VLAN ${v.vid} « ${v.name} » ?`, text: v.ports ? `${v.ports} port${v.ports > 1 ? 's le perdront' : ' le perdra'}.` : undefined, confirmLabel: 'Supprimer', danger: true,
      run: () => api.post(`/projects/${pid}/network/vlans/delete`, { id: v.id }) });
    if (!ok) return;
    toast(`VLAN ${v.vid} supprimé.`);
    await load();
  }
  /** Attributs d'erreur d'un champ du dialogue VLAN (FieldError porte l'id « {id}-err »). */
  const errAttr = (field: string, id: string) => (errs?.field(field) ? { 'aria-invalid': true, 'aria-describedby': `${id}-err` } : {});

  // ----- Multicast -----
  async function assign(mode: 'missing' | 'all') {
    if (mode === 'all' && !(await ask({ title: 'Redistribuer tous les groupes ?', text: 'Les saisies manuelles sont gardées.', confirmLabel: 'Redistribuer' }))) return;
    try { const x = await api.post<{ assigned: number }>(`/projects/${pid}/network/flows/assign`, { mode }); toast(`${x.assigned} groupe${x.assigned > 1 ? 's' : ''} attribué${x.assigned > 1 ? 's' : ''}.`); await load(); } catch (e) { fail(e); }
  }
  async function setFlow(f: Flow, address: string, udp?: number) {
    if (!address.trim()) { await clearFlow(f); return; }
    try { await api.post(`/projects/${pid}/network/flows/set`, { key: f.key, address, udp: udp ?? f.udp ?? undefined }); await load(); } catch (e) { fail(e); await load(); }
  }
  /** Effacer le groupe d'un flux : immédiat, avec Annuler dans la notification. */
  async function clearFlow(f: Flow) {
    const prev = { key: f.key, address: f.address, udp: f.udp };
    try { await api.post(`/projects/${pid}/network/flows/clear`, { keys: [f.key] }); await load(); } catch (e) { fail(e); await load(); return; }
    if (!prev.address) return;
    toast(`Groupe ${prev.address} effacé.`, 'ok', { run: async () => {
      try { await api.post(`/projects/${pid}/network/flows/set`, { key: prev.key, address: prev.address, udp: prev.udp ?? undefined }); } finally { await load(); }
    } });
  }
  async function clearFlows() {
    if (!(await ask({ title: 'Effacer tous les groupes multicast ?', text: 'Les saisies manuelles sont effacées aussi.', confirmLabel: 'Tout effacer', danger: true,
      run: () => api.post(`/projects/${pid}/network/flows/clear`, {}) }))) return;
    toast('Groupes multicast effacés.');
    await load();
  }

  // ----- Carte -----
  const NW = 210, NH = 64;
  const layout = $derived.by(() => {
    if (!r) return { pos: new Map<string, { x: number; y: number }>(), w: 0, h: 0 };
    const sw = r.map.nodes.filter((n) => n.switch), ot = r.map.nodes.filter((n) => !n.switch);
    const pos = new Map<string, { x: number; y: number }>();
    // Équipements terminaux répartis à gauche et à droite, triés selon leur switch.
    const swIdx = new Map(sw.map((n, i) => [n.key, i]));
    const anchor = (k: string) => { const l = r!.map.links.find((x) => x.a === k || x.b === k); const o = l ? (l.a === k ? l.b : l.a) : ''; return swIdx.get(o) ?? 99; };
    const sorted = [...ot].sort((a, b) => anchor(a.key) - anchor(b.key) || a.label.localeCompare(b.label));
    const left = sorted.filter((_, i) => i % 2 === 0), right = sorted.filter((_, i) => i % 2 === 1);
    const rows = Math.max(left.length, right.length, sw.length * 2, 1);
    const h = rows * (NH + 24) + 40;
    const col = (list: MapNode[], x: number) => { const step = (h - 40) / Math.max(list.length, 1); list.forEach((n, i) => pos.set(n.key, { x, y: 20 + step * i + (step - NH) / 2 })); };
    const cols = sw.length ? 3 : 2;
    const w = cols === 3 ? 3 * NW + 2 * 160 + 40 : 2 * NW + 200 + 40;
    col(left, 20);
    if (sw.length) { col(sw, 20 + NW + 160); col(right, 20 + 2 * NW + 320); } else col(right, 20 + NW + 200);
    return { pos, w, h };
  });
  // Carte : liaisons regroupées par paire d'équipements (épaisseur selon le nombre), filtre VLAN.
  const mapLinks = $derived.by(() => {
    if (!r) return [] as (MapLink & { count: number })[];
    const g = new Map<string, MapLink & { count: number }>();
    for (const l of r.map.links.filter((x) => mapVlan === '' || x.vlans.includes(mapVlan as number))) {
      const k = [l.a, l.b].sort().join('|'); const e = g.get(k);
      if (e) { e.count++; e.trunk ||= l.trunk; e.vlans = [...new Set([...e.vlans, ...l.vlans])]; } else g.set(k, { ...l, count: 1 });
    }
    return [...g.values()];
  });
  // D'où viennent les flux : les sorties ST 2110 déclarées dans les fiches des équipements.
  const flowEquip = $derived([...new Map((r?.flows ?? []).map((f) => [f.instance, `${f.equipment} ${f.tag}`])).values()]);
  const flowCount = $derived(flowEquip.length);
  const flowSources = $derived(flowCount === 1 ? flowEquip[0]! : `${flowCount} équipements (${flowEquip.slice(0, 3).join(', ')}${flowCount > 3 ? ` et ${flowCount - 3} autre${flowCount - 3 > 1 ? 's' : ''}` : ''})`);
  const flowHint = $derived(r ? `Flux ST 2110 déclarés dans les fiches du catalogue des équipements du projet (${r.stats.flows}), indépendamment des ports câblés ; « adressés » : avec un groupe multicast attribué (${r.stats.flows_assigned}).` : '');
  const empty = $derived(!!r && !r.ports.length && !r.vlans.length && !r.stats.flows_assigned);
  // Réseaux redondants A et B : jetons du thème (bleu et ambre), sinon couleur du VLAN unique.
  const NET_A = 'var(--sig-v)', NET_B = 'var(--sig-l)';
  const linkColor = (l: MapLink) => l.redundancy === 'A' ? NET_A : l.redundancy === 'B' ? NET_B : (l.vlans.length === 1 ? vlanOf(l.vlans[0]!)?.color : null) ?? 'var(--fg-3)';
  const tabs = $derived(r ? [
    { value: 'ports' as Tab, label: 'Ports', count: r.ports.length },
    { value: 'vlans' as Tab, label: 'VLAN', count: r.vlans.length },
    { value: 'flows' as Tab, label: 'Multicast 2110', count: r.flows.length },
    { value: 'map' as Tab, label: 'Carte', count: r.map.nodes.length },
  ] : []);
</script>

{#if !r}<Loading label="Chargement du réseau" />
{:else}
  <section class="card nw-head">
    <div class="nw-stats">
      <div><b>{r.vlans.length}</b><span class="muted small">VLAN</span></div>
      <div><b>{r.stats.with_ip}<span class="muted">/{r.stats.ports}</span></b><span class="muted small">ports adressés</span></div>
      <div title={flowHint}><b>{r.stats.flows_assigned}<span class="muted">/{r.stats.flows}</span></b><span class="muted small">flux multicast adressés <span class="nw-q" aria-hidden="true">?</span></span></div>
      <div class:bad={r.issues.length}><b>{r.issues.length}</b><span class="muted small">anomalie{r.issues.length > 1 ? 's' : ''}</span></div>
    </div>
    <div class="nw-act">
      {#if writable}<SaveState />{/if}
      <span title={empty ? 'Rien à exporter : aucun port réseau, aucun VLAN et aucun flux adressé.' : undefined}><button class="btn sm ghost" disabled={empty} onclick={() => download(`/projects/${pid}/network/export?format=csv`)}><Icon name="download" size="sm" />CSV</button></span>
      <span title={empty ? 'Rien à exporter : aucun port réseau, aucun VLAN et aucun flux adressé.' : undefined}><button class="btn sm ghost" disabled={empty} onclick={() => download(`/projects/${pid}/network/export?format=xlsx`)}><Icon name="download" size="sm" />Plan IP (Excel)</button></span>
    </div>
    {#if r.stats.flows && !r.stats.ports}<p class="muted small nw-note">Les {r.stats.flows} flux viennent des fiches du catalogue : {flowSources} déclare{flowCount > 1 ? 'nt' : ''} des sorties ST 2110, même si aucun de ses ports n’est encore câblé en réseau.</p>{/if}
  </section>

  {#if r.issues.length}
    <details class="card compact nw-issues" open={r.issues.length <= 4}>
      <summary><Icon name="alert" size="sm" />{r.issues.length} point{r.issues.length > 1 ? 's' : ''} à corriger</summary>
      <ul>{#each r.issues as i}<li>{i.message}</li>{/each}</ul>
    </details>
  {/if}

  <Seg class="nw-tabs" mode="tabs" label="Vues du réseau" controls="nw" options={tabs} value={tab} onchange={setTab} />

  <div role="tabpanel" id="nw-{tab}" aria-label={tabs.find((t) => t.value === tab)?.label}>
  {#if tab === 'ports'}
    {#if !r.ports.length}
      <EmptyState icon="network" message="Aucun port réseau" hint="Les ports dont le signal est IP (Ethernet, Dante, ST 2110) des équipements du projet apparaîtront ici.">
        {#snippet action()}<a class="btn" class:primary={writable} href="/projets/{pid}/inventaire"><Icon name="box" size="sm" />Ajouter du matériel</a>{/snippet}
      </EmptyState>
    {:else}
      <div class="ct-bar">
        <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={q} placeholder="Équipement, port, IP, nom d’hôte" aria-label="Rechercher un port"></label>
        <select class="input nw-sel" bind:value={vf} aria-label="VLAN"><option value="">Tous les VLAN</option><option value="none">Sans VLAN</option>{#each r.vlans as v}<option value={v.vid}>{v.vid} · {v.name}</option>{/each}</select>
      </div>
      {#if writable}
        <div class="ct-bar nw-bulk">
          <span class="small"><b>{selRefs.length}</b> port{selRefs.length > 1 ? 's' : ''} choisi{selRefs.length > 1 ? 's' : ''}</span>
          <select class="input sm nw-sel" aria-label="Mode des ports choisis" disabled={!selRefs.length || busy} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v) setPorts(selRefs, { mode: v }); (e.target as HTMLSelectElement).value = ''; }}><option value="">Mode</option><option value="access">Accès</option><option value="trunk">Trunk</option></select>
          <select class="input sm nw-sel" aria-label="VLAN des ports choisis" disabled={!selRefs.length || busy} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v) setPorts(selRefs, { vlan: v === 'none' ? null : Number(v) }); (e.target as HTMLSelectElement).value = ''; }}><option value="">VLAN</option><option value="none">Aucun</option>{#each r.vlans as v}<option value={v.vid}>{v.vid} · {v.name}</option>{/each}</select>
          <select class="input sm nw-sel" aria-label="Réseau des ports choisis" disabled={!selRefs.length || busy} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v) setPorts(selRefs, { redundancy: v === 'none' ? null : v }); (e.target as HTMLSelectElement).value = ''; }}><option value="">Réseau</option><option value="A">Réseau A</option><option value="B">Réseau B</option><option value="none">Aucun</option></select>
          <button class="btn sm" disabled={!selRefs.length || busy} onclick={() => autoIp(selRefs)}>Adresser</button>
          <button class="btn sm ghost" disabled={!selRefs.length || busy} onclick={() => setPorts(selRefs, { ip: null })}>Effacer les IP</button>
          <span style="flex:1"></span>
          <button class="btn sm primary" disabled={busy} onclick={() => autoIp()} title="Première adresse libre du sous-réseau de chaque VLAN, hors passerelle">{#if busy}<span class="spin" aria-hidden="true"></span>{:else}<Icon name="zap" size="sm" />{/if}Adresser tous les ports</button>
        </div>
      {/if}
      <section class="card tbl-card dense">
        <div class="tbl-wrap"><table class="tbl dense ct-tbl nw-tbl">
          <thead><tr>
            {#if writable}<th class="ck"><input type="checkbox" checked={allSel} onchange={selAll} aria-label="Tout choisir"></th>{/if}
            <SortTh label="Équipement" key="equipment" bind:sort /><th>Port</th><th>Mode</th><SortTh label="VLAN" key="vlan" bind:sort /><SortTh label="Adresse IP" key="ip" bind:sort /><th>Nom d’hôte</th><th>Protocole</th><th>Réseau</th><th>Relié à</th>
          </tr></thead>
          <tbody>
            {#each shown as p (p.ref)}
              {@const v = vlanOf(p.vlan)}
              <tr class:sel={sel.has(p.ref)} class:warn={issueRefs.has(p.ref)}>
                {#if writable}<td class="ck"><input type="checkbox" checked={sel.has(p.ref)} onchange={() => toggleSel(p.ref)} aria-label="Choisir {p.equipment} {p.port}"></td>{/if}
                <td class="end"><b>{p.equipment} <span class="mono muted">{p.tag}</span>{#if p.switching}<span class="chip">switch</span>{/if}</b><span class="muted small">{p.element}</span></td>
                <td><b class="small">{p.port}</b></td>
                <td><select class="cell" value={p.mode} disabled={!writable} aria-label="Mode" onchange={(e) => setPorts([p.ref], { mode: (e.target as HTMLSelectElement).value })}><option value="access">Accès</option><option value="trunk">Trunk</option></select></td>
                <td>
                  <span class="vl">
                    {#if v}<i class="dot" style="--c:{v.color}"></i>{/if}
                    <select class="cell" value={p.vlan ?? ''} disabled={!writable} aria-label={p.mode === 'trunk' ? 'VLAN natif' : 'VLAN'} onchange={(e) => { const x = (e.target as HTMLSelectElement).value; setPorts([p.ref], { vlan: x ? Number(x) : null }); }}>
                      <option value="">{p.mode === 'trunk' ? 'Natif : aucun' : '—'}</option>{#each r.vlans as o}<option value={o.vid}>{p.mode === 'trunk' ? 'Natif ' : ''}{o.vid} · {o.name}</option>{/each}
                    </select>
                  </span>
                  {#if p.mode === 'trunk'}
                    <span class="trunk">{#each r.vlans as o}<label class="tv" style="--c:{o.color}" title={o.name}><input type="checkbox" checked={p.vlans.includes(o.vid)} disabled={!writable} onchange={(e) => setPorts([p.ref], { vlans: (e.target as HTMLInputElement).checked ? [...p.vlans, o.vid] : p.vlans.filter((x) => x !== o.vid) })}>{o.vid}</label>{/each}</span>
                  {/if}
                </td>
                <td><input class="cell mono" value={p.ip ?? ''} placeholder={v?.cidr ?? '—'} disabled={!writable} aria-label="Adresse IP" onchange={(e) => setPorts([p.ref], { ip: (e.target as HTMLInputElement).value.trim() || null })}></td>
                <td><input class="cell mono" value={p.hostname ?? ''} placeholder="—" disabled={!writable} aria-label="Nom d’hôte" onchange={(e) => setPorts([p.ref], { hostname: (e.target as HTMLInputElement).value.trim() || null })}></td>
                <td><select class="cell" value={p.protocol ?? ''} disabled={!writable} aria-label="Protocole" onchange={(e) => setPorts([p.ref], { protocol: (e.target as HTMLSelectElement).value || null })}><option value="">—</option>{#each PROTO as [k, l]}<option value={k}>{l}</option>{/each}</select></td>
                <td><select class="cell" value={p.redundancy ?? ''} disabled={!writable} aria-label="Réseau" onchange={(e) => setPorts([p.ref], { redundancy: (e.target as HTMLSelectElement).value || null })}><option value="">—</option><option value="A">A</option><option value="B">B</option></select></td>
                <td class="small">{p.peer ?? ''}</td>
              </tr>
            {:else}
              <tr><td colspan={writable ? 10 : 9}><EmptyState card={false} icon="search" message="Aucun port ne correspond à la recherche." /></td></tr>
            {/each}
          </tbody>
        </table></div>
      </section>
    {/if}

  {:else if tab === 'vlans'}
    <div class="ct-bar"><button type="button" class="ux3-info" title="Numéro unique de 1 à 4094. Le sous-réseau sert à l’adressage automatique." aria-label="Numéro unique de 1 à 4094. Le sous-réseau sert à l’adressage automatique."><Icon name="info" size="sm" /></button><span style="flex:1"></span>{#if writable && r.vlans.length}<button class="btn sm primary" onclick={newVlan}><Icon name="plus" size="sm" />Nouveau VLAN</button>{/if}</div>
    {#if !r.vlans.length}
      <EmptyState icon="network" message="Aucun VLAN" hint="Créez les VLAN du projet (production, contrôle, Dante, gestion), puis affectez-les aux ports.">
        {#snippet action()}{#if writable}<button class="btn primary" onclick={newVlan}><Icon name="plus" size="sm" />Nouveau VLAN</button>{/if}{/snippet}
      </EmptyState>
    {:else}
      <div class="vgrid">
        {#each r.vlans as v (v.id)}
          <article class="card vcard" style="--c:{v.color}">
            <div class="vtop"><span class="vid mono">{v.vid}</span><div class="vt"><b>{v.name}</b><span class="muted small">{v.role ?? 'Rôle non précisé'}</span></div>
              {#if writable}<button class="icon-btn sm" aria-label="Modifier le VLAN {v.vid}" title="Modifier le VLAN {v.vid}" data-focus-key="vlan-edit-{v.id}" onclick={() => editVlan(v)}><Icon name="pen" size="sm" /></button><button class="icon-btn sm danger" aria-label="Supprimer le VLAN {v.vid}" title="Supprimer le VLAN {v.vid}" onclick={() => delVlan(v)}><Icon name="trash" size="sm" /></button>{/if}
            </div>
            <dl class="vdl">
              <dt>Sous-réseau</dt><dd class="mono">{v.cidr ?? '—'}</dd>
              <dt>Passerelle</dt><dd class="mono">{v.gateway ?? '—'}</dd>
              <dt>Ports</dt><dd>{v.ports}</dd>
            </dl>
            {#if v.hosts != null}
              <div class="bar" title="{v.used ?? 0} adresse{pl(v.used ?? 0)} utilisée{pl(v.used ?? 0)} sur {v.hosts}"><i style="width:{Math.min(100, ((v.used ?? 0) / Math.max(v.hosts, 1)) * 100)}%"></i></div>
              <span class="muted small">{v.used} / {v.hosts} adresses</span>
            {/if}
          </article>
        {/each}
      </div>
    {/if}

  {:else if tab === 'flows'}
    <div class="ct-bar">
      <span class="muted small">Groupes <span class="mono">239.20.x.y</span> vidéo, <span class="mono">239.30</span> audio, <span class="mono">239.40</span> données · UDP 50020 / 50030 / 50040. La saisie manuelle prime.</span>
      <span style="flex:1"></span>
      {#if writable && r.flows.length}
        <button class="btn sm ghost" onclick={() => clearFlows()}>Tout effacer</button>
        <button class="btn sm" onclick={() => assign('all')}>Redistribuer</button>
        <button class="btn sm primary" onclick={() => assign('missing')}><Icon name="zap" size="sm" />Attribuer les manquants</button>
      {/if}
    </div>
    {#if !r.flows.length}
      <EmptyState icon="network" message="Aucun flux IP" hint="Déclarez les flux des équipements dans le catalogue (fiche équipement › Flux IP) : chaque sortie reçoit ici un groupe multicast.">
        {#snippet action()}<a class="btn" href="/catalogue">Ouvrir le catalogue</a>{/snippet}
      </EmptyState>
    {:else}
      <section class="card tbl-card dense">
        <div class="tbl-wrap"><table class="tbl dense ct-tbl">
          <thead><tr><th>Équipement</th><th>Flux</th><th>Essence</th><th>Groupe multicast</th><th class="num">UDP</th><th><span class="sr-only">Actions</span></th></tr></thead>
          <tbody>
            {#each r.flows as f (f.key)}
              <tr class:warn={issueRefs.has(f.key)}>
                <td class="end"><b>{f.equipment} <span class="mono muted">{f.tag}</span></b><span class="muted small">{f.element}</span></td>
                <td>{f.signal}</td>
                <td><span class="chip fn"><i class="dot" style="--c:{familyColor(ESS_FAMILY[f.essence], { screen: true })}"></i>{ESS[f.essence]}</span></td>
                <td><span class="mc"><input class="cell mono" value={f.address ?? ''} placeholder="non attribué" disabled={!writable} aria-label="Groupe multicast" onchange={(e) => setFlow(f, (e.target as HTMLInputElement).value.trim())}>{#if f.manual}<span class="chip" title="Saisie manuelle : gardée lors d’une redistribution">manuel</span>{/if}</span></td>
                <td class="num"><input class="cell mono udp" value={f.udp ?? ''} disabled={!writable || !f.address} aria-label="Port UDP" onchange={(e) => setFlow(f, f.address ?? '', Number((e.target as HTMLInputElement).value))}></td>
                <td class="acts">{#if writable && f.address}<button class="icon-btn sm" aria-label="Effacer le groupe {f.address}" title="Effacer le groupe {f.address}" onclick={() => clearFlow(f)}><Icon name="x" size="sm" /></button>{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table></div>
      </section>
    {/if}

  {:else}
    {#if !r.map.nodes.length}
      <EmptyState icon="network" message="Carte vide" hint="Elle se dessine à partir des équipements à ports IP et de leurs câbles.">
        {#snippet action()}<a class="btn" href="/projets/{pid}/synoptique"><Icon name="syn" size="sm" />Ouvrir le synoptique</a>{/snippet}
      </EmptyState>
    {:else}
      <div class="ct-bar small muted"><select class="input sm nw-sel" bind:value={mapVlan} aria-label="Filtrer par VLAN"><option value="">Tous les VLAN</option>{#each r.vlans as v}<option value={v.vid}>VLAN {v.vid} · {v.name}</option>{/each}</select><span class="lg"><i style="background:{NET_A}"></i>Réseau A</span><span class="lg"><i style="background:{NET_B}"></i>Réseau B</span><span class="lg"><i class="dash"></i>Trunk</span>{#each r.vlans as v}<span class="lg"><i style="background:{v.color}"></i>VLAN {v.vid}</span>{/each}</div>
      <section class="card nw-map">
        <svg viewBox="0 0 {layout.w} {layout.h}" width={layout.w} height={layout.h} role="img" aria-label="Carte du réseau">
          {#each mapLinks as l}
            {@const a = layout.pos.get(l.a)}{@const b = layout.pos.get(l.b)}
            {#if a && b}
              {@const ax = a.x < b.x ? a.x + NW : a.x}{@const bx = a.x < b.x ? b.x : b.x + NW}
              <path d="M{ax} {a.y + NH / 2} C{(ax + bx) / 2} {a.y + NH / 2} {(ax + bx) / 2} {b.y + NH / 2} {bx} {b.y + NH / 2}" fill="none" stroke={linkColor(l)} stroke-width={Math.min(10, (l.trunk ? 3 : 2) + (l.count - 1) * 1.5)} stroke-dasharray={l.trunk ? '7 4' : undefined}><title>{l.count > 1 ? `${l.count} liaisons · ` : ''}{l.a_port} ↔ {l.b_port}{l.vlans.length ? ` · VLAN ${l.vlans.join(', ')}` : ''}</title></path>
            {/if}
          {/each}
          {#each r.map.nodes as n (n.key)}
            {@const p = layout.pos.get(n.key)}
            {#if p}
              <g transform="translate({p.x} {p.y})">
                <rect width={NW} height={NH} rx="10" class="nd" class:swn={n.switch} />
                {#each n.vlans.slice(0, 8) as vid, i}<rect x={10 + i * 12} y={NH - 12} width="9" height="5" rx="2" fill={vlanOf(vid)?.color ?? 'var(--fg-3)'}><title>VLAN {vid}</title></rect>{/each}
                <text x="10" y="20" class="nt">{n.label.length > 26 ? n.label.slice(0, 25) + '…' : n.label}</text>
                <text x="10" y="35" class="ns">{n.tag} · {n.element.length > 18 ? n.element.slice(0, 17) + '…' : n.element}</text>
                <text x={NW - 10} y="20" class="ns" text-anchor="end">{n.switch ? 'SWITCH' : `${n.ports} port${n.ports > 1 ? 's' : ''}`}</text>
                {#if n.ips.length}<text x={NW - 10} y={NH - 8} class="ns mono" text-anchor="end">{n.ips[0]}{n.ips.length > 1 ? ` +${n.ips.length - 1}` : ''}</text>{/if}
              </g>
            {/if}
          {/each}
        </svg>
      </section>
    {/if}
  {/if}
  </div>
{/if}

{#if edit}
  <Dialog title={edit.id ? `VLAN ${edit.vid}` : 'Nouveau VLAN'} eyebrow="Réseau" onclose={() => (edit = null)} onsubmit={saveVlan} submitLabel={edit.id ? 'Enregistrer' : 'Créer'}>
    <div class="fg2">
      <div class="field"><label for="v-vid">Numéro</label><input class="input mono" id="v-vid" type="number" min="1" max="4094" bind:value={edit.vid} {...errAttr('vid', 'v-vid')}><FieldError for="v-vid" message={errs?.field('vid')} /></div>
      <div class="field"><label for="v-name">Nom</label><input class="input" id="v-name" bind:value={edit.name} placeholder="Production 2110" {...errAttr('name', 'v-name')}><FieldError for="v-name" message={errs?.field('name')} /></div>
      <div class="field"><label for="v-cidr">Sous-réseau</label><input class="input mono" id="v-cidr" bind:value={edit.cidr} placeholder="10.10.20.0/24" {...errAttr('cidr', 'v-cidr')}><FieldError for="v-cidr" message={errs?.field('cidr')} /></div>
      <div class="field"><label for="v-gw">Passerelle</label><input class="input mono" id="v-gw" bind:value={edit.gateway} placeholder="10.10.20.1" {...errAttr('gateway', 'v-gw')}><FieldError for="v-gw" message={errs?.field('gateway')} /></div>
      <div class="field"><label for="v-role">Rôle</label><input class="input" id="v-role" bind:value={edit.role} list="v-roles" placeholder="Gestion, média principal, Dante"><datalist id="v-roles">{#each ['Gestion', 'Média principal', 'Média secondaire', 'Dante', 'Autre'] as x}<option value={x}></option>{/each}</datalist></div>
      <div class="field"><span class="flabel" id="v-color">Couleur</span><div class="sw-row" role="group" aria-labelledby="v-color">{#each PALETTE as c, i}<button type="button" class="swc" class:on={edit.color === c} aria-pressed={edit.color === c} style="--c:{c}" aria-label="Couleur {i + 1}" title="Couleur {i + 1}" onclick={() => (edit!.color = c)}></button>{/each}</div><FieldError for="v-color" message={errs?.field('color')} /></div>
    </div>
  </Dialog>
{/if}

<style>
  .nw-head { padding: 16px 20px; display: flex; gap: 20px; align-items: center; justify-content: space-between; flex-wrap: wrap; margin-bottom: 12px; }
  .nw-stats { display: flex; gap: 28px; flex-wrap: wrap; }
  .nw-stats > div { display: flex; flex-direction: column; }
  .nw-stats b { font-size: 26px; line-height: 1.1; letter-spacing: -.01em; }
  .nw-stats .bad b { color: var(--err); }
  .nw-act { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  .nw-note { flex-basis: 100%; margin: 0; }
  .nw-q { display: inline-grid; place-items: center; width: 14px; height: 14px; border-radius: 50%; border: 1px solid currentColor; font-size: 9.5px; cursor: help; }
  .nw-issues { margin-bottom: 12px; box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--err) 40%, var(--line)), var(--card-sh); }
  .nw-issues summary { cursor: pointer; font-weight: 600; display: flex; gap: 8px; align-items: center; color: var(--err); }
  .nw-issues ul { margin: 8px 0 2px; padding-left: 20px; display: grid; gap: 4px; }
  :global(.seg.nw-tabs) { margin-bottom: 12px; }
  .ct-bar { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; margin-bottom: 10px; }
  .nw-sel { width: auto; }
  .nw-bulk { padding: 8px 12px; border-radius: 12px; background: var(--surface-2); }
  .ct-tbl tr.warn td:first-child, .ct-tbl tr.warn td:first-child + td { box-shadow: inset 3px 0 0 var(--err); }
  .end { display: table-cell; }
  .end b { display: block; }
  .end .chip { margin-left: 6px; }
  /* Largeurs minimales des cellules : leur apparence (.cell) est commune (harmonie.css). */
  .ct-tbl td .cell { min-width: 70px; }
  .ct-tbl td select.cell { min-width: 90px; }
  .ct-tbl td .cell.udp { width: 80px; }
  .dot { display: inline-block; width: 8px; height: 8px; border-radius: 50%; background: var(--c); flex: none; }
  .vl { display: flex; align-items: center; gap: 4px; }
  .mc { display: flex; align-items: center; gap: 6px; }
  .trunk { display: flex; gap: 4px; flex-wrap: wrap; margin-top: 4px; }
  .tv { display: inline-flex; gap: 3px; align-items: center; font: 600 11.5px var(--mono); padding: 1px 6px; border-radius: 99px; border: 1.5px solid var(--c); }
  .tv input { width: 12px; height: 12px; margin: 0; }
  .vgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .vcard { padding: 14px 16px; border-top: 4px solid var(--c); display: grid; gap: 8px; }
  .vtop { display: flex; gap: 10px; align-items: center; }
  .vid { font-size: 22px; font-weight: 700; color: var(--c); min-width: 48px; }
  .vt { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .vdl { display: grid; grid-template-columns: auto 1fr; gap: 2px 12px; margin: 0; }
  .vdl dt { color: var(--fg-2); }
  .vdl dd { margin: 0; }
  .bar { height: 6px; border-radius: 99px; background: var(--surface-2); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--c); }
  .sw-row { display: flex; gap: 6px; flex-wrap: wrap; }
  .swc { width: 26px; height: 26px; border-radius: 50%; background: var(--c); border: 2px solid transparent; cursor: pointer; }
  .swc.on { border-color: var(--fg); box-shadow: inset 0 0 0 1.5px var(--bg); }
  .nw-map { padding: 12px; overflow: auto; }
  .nw-map svg { display: block; max-width: none; }
  .nd { fill: var(--surface); stroke: var(--line-strong, var(--line)); stroke-width: 1.5; }
  .nd.swn { fill: color-mix(in srgb, var(--fg) 6%, var(--surface)); stroke: var(--fg); }
  .nt { font: 600 13px var(--font); fill: var(--fg); }
  .ns { font: 500 11px var(--font); fill: var(--fg-2); }
  .ns.mono { font-family: var(--mono); }
  .lg { display: inline-flex; gap: 6px; align-items: center; }
  .lg i { width: 18px; height: 4px; border-radius: 2px; display: inline-block; }
  .lg i.dash { background: repeating-linear-gradient(90deg, var(--fg-2) 0 6px, transparent 6px 9px); }
  @media (max-width: 700px) { .nw-stats { gap: 16px; } .nw-stats b { font-size: 20px; } }
</style>
