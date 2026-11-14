<script lang="ts">
  import { ask } from '../../lib/ask.svelte';
  // Réseau (section 11.3) : VLAN, ports réseau (accès ou trunk, IP, nom d'hôte, réseau A/B),
  // plan multicast SMPTE ST 2110 et carte du réseau. Écran partagé en direct.
  import { untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import Dialog from '../../components/Dialog.svelte';
  import { api, download, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import type { ProjectView } from '../../lib/projects';

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
  const PROTO: [string, string][] = [['st2110-20', 'ST 2110-20'], ['st2110-30', 'ST 2110-30'], ['st2110-40', 'ST 2110-40'], ['dante', 'Dante'], ['ndi', 'NDI'], ['madi-ip', 'MADI-IP'], ['generic', 'Générique']];
  let mapVlan = $state<number | ''>('');

  let r = $state<R | null>(null);
  let tab = $state<'ports' | 'vlans' | 'flows' | 'map'>('ports');
  let q = $state('');
  let vf = $state<'' | 'none' | number>('');
  let picked = $state(new Set<string>());
  let busy = $state(false);

  async function load() { try { r = await api.get(`/projects/${pid}/network`); } catch (e) { fail(e); } }
  $effect(() => { view; untrack(load); });

  const vlanOf = (vid: number | null) => r?.vlans.find((v) => v.vid === vid);
  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const issueRefs = $derived(new Set(r?.issues.flatMap((i) => i.refs) ?? []));
  const shown = $derived((r?.ports ?? []).filter((p) =>
    (!q || norm(`${p.element} ${p.equipment} ${p.tag} ${p.port} ${p.ip ?? ''} ${p.hostname ?? ''}`).includes(norm(q)))
    && (vf === '' || (vf === 'none' ? p.vlan == null && !p.vlans.length : p.vlan === vf || p.vlans.includes(vf as number)))));
  const allPicked = $derived(shown.length > 0 && shown.every((p) => picked.has(p.ref)));
  function pickAll() { const s = new Set(picked); if (allPicked) shown.forEach((p) => s.delete(p.ref)); else shown.forEach((p) => s.add(p.ref)); picked = s; }
  function togglePick(ref: string) { const s = new Set(picked); if (s.has(ref)) s.delete(ref); else s.add(ref); picked = s; }

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
  const sel = $derived([...picked].filter((x) => r?.ports.some((p) => p.ref === x)));

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
      toast(edit.id ? 'VLAN mis à jour.' : 'VLAN ajouté.'); edit = null; await load();
    } catch (e) { if (e instanceof ApiError && e.fields.length) errs = e; else fail(e); }
  }
  async function delVlan(v: Vlan) {
    if (!(await ask({ title: `Supprimer le VLAN ${v.vid} « ${v.name} » ?`, text: v.ports ? `${v.ports} port${v.ports > 1 ? 's le perdront' : ' le perdra'}.` : undefined, confirmLabel: 'Supprimer', danger: true }))) return;
    try { await api.post(`/projects/${pid}/network/vlans/delete`, { id: v.id }); await load(); } catch (e) { fail(e); }
  }

  // ----- Multicast -----
  async function assign(mode: 'missing' | 'all') {
    if (mode === 'all' && !(await ask({ title: 'Redistribuer tous les groupes ?', text: 'Les saisies manuelles sont gardées.', confirmLabel: 'Redistribuer' }))) return;
    try { const x = await api.post<{ assigned: number }>(`/projects/${pid}/network/flows/assign`, { mode }); toast(`${x.assigned} groupe${x.assigned > 1 ? 's' : ''} attribué${x.assigned > 1 ? 's' : ''}.`); await load(); } catch (e) { fail(e); }
  }
  async function setFlow(f: Flow, address: string, udp?: number) {
    if (!address.trim()) { await clearFlows([f.key]); return; }
    try { await api.post(`/projects/${pid}/network/flows/set`, { key: f.key, address, udp: udp ?? f.udp ?? undefined }); await load(); } catch (e) { fail(e); await load(); }
  }
  async function clearFlows(keys?: string[]) {
    if (!keys && !(await ask({ title: 'Effacer tous les groupes multicast ?', text: 'Les saisies manuelles sont effacées aussi.', confirmLabel: 'Tout effacer', danger: true }))) return;
    try { await api.post(`/projects/${pid}/network/flows/clear`, keys ? { keys } : {}); await load(); } catch (e) { fail(e); }
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
  const linkColor = (l: MapLink) => l.redundancy === 'A' ? '#3A7BD5' : l.redundancy === 'B' ? '#E0A030' : (l.vlans.length === 1 ? vlanOf(l.vlans[0]!)?.color : null) ?? 'var(--fg-3)';
</script>

{#if !r}<span class="spin"></span>
{:else}
  <section class="card nw-head">
    <div class="nw-stats">
      <div><b>{r.vlans.length}</b><span class="muted small">VLAN</span></div>
      <div><b>{r.stats.with_ip}<span class="muted">/{r.stats.ports}</span></b><span class="muted small">ports adressés</span></div>
      <div><b>{r.stats.flows_assigned}<span class="muted">/{r.stats.flows}</span></b><span class="muted small">flux multicast</span></div>
      <div class:bad={r.issues.length}><b>{r.issues.length}</b><span class="muted small">anomalie{r.issues.length > 1 ? 's' : ''}</span></div>
    </div>
    <div class="nw-act">
      <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/network/export?format=csv`)}><Icon name="download" size="sm" />CSV</button>
      <button class="btn sm ghost" onclick={() => download(`/projects/${pid}/network/export?format=xlsx`)}><Icon name="download" size="sm" />Plan IP XLSX</button>
    </div>
  </section>

  {#if r.issues.length}
    <details class="card nw-issues" open={r.issues.length <= 4}>
      <summary><Icon name="alert" size="sm" />{r.issues.length} point{r.issues.length > 1 ? 's' : ''} à corriger</summary>
      <ul>{#each r.issues as i}<li>{i.message}</li>{/each}</ul>
    </details>
  {/if}

  <div class="seg nw-tabs" role="tablist" aria-label="Vues du réseau">
    {#each [['ports', 'Ports', r.ports.length], ['vlans', 'VLAN', r.vlans.length], ['flows', 'Multicast 2110', r.flows.length], ['map', 'Carte', r.map.nodes.length]] as [k, l, n]}
      <button role="tab" aria-selected={tab === k} class:on={tab === k} onclick={() => (tab = k as typeof tab)}>{l} <span class="muted mono">{n}</span></button>
    {/each}
  </div>

  {#if tab === 'ports'}
    {#if !r.ports.length}
      <section class="card empty-card"><h3>Aucun port réseau</h3><p class="muted">Les ports dont le signal est IP (Ethernet, Dante, ST 2110…) des équipements du projet apparaîtront ici.</p></section>
    {:else}
      <div class="ct-bar">
        <label class="search" style="flex:1;min-width:220px"><Icon name="search" /><input class="input" bind:value={q} placeholder="Équipement, port, IP, nom d’hôte…" aria-label="Rechercher un port"></label>
        <select class="input" bind:value={vf} aria-label="VLAN"><option value="">Tous les VLAN</option><option value="none">Sans VLAN</option>{#each r.vlans as v}<option value={v.vid}>{v.vid} · {v.name}</option>{/each}</select>
      </div>
      {#if writable}
        <div class="ct-bar nw-bulk" class:on={sel.length}>
          <span class="small"><b>{sel.length}</b> port{sel.length > 1 ? 's' : ''} choisi{sel.length > 1 ? 's' : ''}</span>
          <select class="input" aria-label="Mode des ports choisis" disabled={!sel.length || busy} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v) setPorts(sel, { mode: v }); (e.target as HTMLSelectElement).value = ''; }}><option value="">Mode…</option><option value="access">Accès</option><option value="trunk">Trunk</option></select>
          <select class="input" aria-label="VLAN des ports choisis" disabled={!sel.length || busy} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v) setPorts(sel, { vlan: v === 'none' ? null : Number(v) }); (e.target as HTMLSelectElement).value = ''; }}><option value="">VLAN…</option><option value="none">Aucun</option>{#each r.vlans as v}<option value={v.vid}>{v.vid} · {v.name}</option>{/each}</select>
          <select class="input" aria-label="Réseau des ports choisis" disabled={!sel.length || busy} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v) setPorts(sel, { redundancy: v === 'none' ? null : v }); (e.target as HTMLSelectElement).value = ''; }}><option value="">Réseau…</option><option value="A">Réseau A</option><option value="B">Réseau B</option><option value="none">Aucun</option></select>
          <button class="btn sm" disabled={!sel.length || busy} onclick={() => autoIp(sel)}>Adresser</button>
          <button class="btn sm ghost" disabled={!sel.length || busy} onclick={() => setPorts(sel, { ip: null })}>Effacer les IP</button>
          <span style="flex:1"></span>
          <button class="btn sm primary" disabled={busy} onclick={() => autoIp()} title="Première adresse libre du sous-réseau de chaque VLAN, hors passerelle"><Icon name="zap" size="sm" />Adresser tous les ports</button>
        </div>
      {/if}
      <section class="card ct">
        <div class="tbl-wrap"><table class="tbl ct-tbl nw-tbl">
          <thead><tr>
            {#if writable}<th class="ck"><input type="checkbox" checked={allPicked} onchange={pickAll} aria-label="Tout choisir"></th>{/if}
            <th>Équipement</th><th>Port</th><th>Mode</th><th>VLAN</th><th>Adresse IP</th><th>Nom d’hôte</th><th>Protocole</th><th>Réseau</th><th>Relié à</th>
          </tr></thead>
          <tbody>
            {#each shown as p (p.ref)}
              {@const v = vlanOf(p.vlan)}
              <tr class:picked={picked.has(p.ref)} class:warn={issueRefs.has(p.ref)}>
                {#if writable}<td class="ck"><input type="checkbox" checked={picked.has(p.ref)} onchange={() => togglePick(p.ref)} aria-label="Choisir {p.equipment} {p.port}"></td>{/if}
                <td class="end"><b>{p.equipment} <span class="mono muted">{p.tag}</span>{#if p.switching}<span class="chip sm">switch</span>{/if}</b><span class="muted small">{p.element}</span></td>
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
            {/each}
          </tbody>
        </table></div>
      </section>
    {/if}

  {:else if tab === 'vlans'}
    <div class="ct-bar"><span class="muted small">Numéro unique de 1 à 4094. Le sous-réseau sert à l’adressage automatique.</span><span style="flex:1"></span>{#if writable}<button class="btn sm primary" onclick={newVlan}><Icon name="plus" size="sm" />Nouveau VLAN</button>{/if}</div>
    {#if !r.vlans.length}
      <section class="card empty-card"><h3>Aucun VLAN</h3><p class="muted">Créez les VLAN du projet (production, contrôle, Dante, gestion…), puis affectez-les aux ports.</p></section>
    {:else}
      <div class="vgrid">
        {#each r.vlans as v (v.id)}
          <article class="card vcard" style="--c:{v.color}">
            <div class="vtop"><span class="vid mono">{v.vid}</span><div class="vt"><b>{v.name}</b><span class="muted small">{v.role ?? 'Rôle non précisé'}</span></div>
              {#if writable}<button class="icon-btn sm" aria-label="Modifier le VLAN {v.vid}" onclick={() => editVlan(v)}><Icon name="pen" size="sm" /></button><button class="icon-btn sm" aria-label="Supprimer le VLAN {v.vid}" onclick={() => delVlan(v)}><Icon name="trash" size="sm" /></button>{/if}
            </div>
            <dl class="vdl">
              <dt>Sous-réseau</dt><dd class="mono">{v.cidr ?? '—'}</dd>
              <dt>Passerelle</dt><dd class="mono">{v.gateway ?? '—'}</dd>
              <dt>Ports</dt><dd>{v.ports}</dd>
            </dl>
            {#if v.hosts != null}
              <div class="bar" title="{v.used} adresse(s) utilisée(s) sur {v.hosts}"><i style="width:{Math.min(100, ((v.used ?? 0) / Math.max(v.hosts, 1)) * 100)}%"></i></div>
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
      <section class="card empty-card"><h3>Aucun flux IP</h3><p class="muted">Déclarez les flux des équipements dans le catalogue (fiche équipement › Flux IP) : chaque sortie reçoit ici un groupe multicast.</p></section>
    {:else}
      <section class="card ct">
        <div class="tbl-wrap"><table class="tbl ct-tbl">
          <thead><tr><th>Équipement</th><th>Flux</th><th>Essence</th><th>Groupe multicast</th><th class="num">UDP</th><th></th></tr></thead>
          <tbody>
            {#each r.flows as f (f.key)}
              <tr class:warn={issueRefs.has(f.key)}>
                <td class="end"><b>{f.equipment} <span class="mono muted">{f.tag}</span></b><span class="muted small">{f.element}</span></td>
                <td>{f.signal}</td>
                <td><span class="ess {f.essence}">{ESS[f.essence]}</span></td>
                <td><input class="cell mono" value={f.address ?? ''} placeholder="non attribué" disabled={!writable} aria-label="Groupe multicast" onchange={(e) => setFlow(f, (e.target as HTMLInputElement).value.trim())}>{#if f.manual}<span class="chip sm" title="Saisie manuelle : gardée lors d’une redistribution">manuel</span>{/if}</td>
                <td class="num"><input class="cell mono num" style="width:80px" value={f.udp ?? ''} disabled={!writable || !f.address} aria-label="Port UDP" onchange={(e) => setFlow(f, f.address ?? '', Number((e.target as HTMLInputElement).value))}></td>
                <td>{#if writable && f.address}<button class="icon-btn sm" aria-label="Effacer le groupe" onclick={() => clearFlows([f.key])}><Icon name="x" size="sm" /></button>{/if}</td>
              </tr>
            {/each}
          </tbody>
        </table></div>
      </section>
    {/if}

  {:else}
    {#if !r.map.nodes.length}
      <section class="card empty-card"><h3>Carte vide</h3><p class="muted">Elle se dessine à partir des équipements à ports IP et de leurs câbles.</p></section>
    {:else}
      <div class="ct-bar small muted"><select class="input" bind:value={mapVlan} aria-label="Filtrer par VLAN"><option value="">Tous les VLAN</option>{#each r.vlans as v}<option value={v.vid}>VLAN {v.vid} · {v.name}</option>{/each}</select><span class="lg"><i style="background:#3A7BD5"></i>Réseau A</span><span class="lg"><i style="background:#E0A030"></i>Réseau B</span><span class="lg"><i class="dash"></i>Trunk</span>{#each r.vlans as v}<span class="lg"><i style="background:{v.color}"></i>VLAN {v.vid}</span>{/each}</div>
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
{/if}

{#if edit}
  <Dialog title={edit.id ? `VLAN ${edit.vid}` : 'Nouveau VLAN'} eyebrow="Réseau" onclose={() => (edit = null)}>
    <div class="fg2">
      <div class="field"><label for="v-vid">Numéro</label><input class="input mono" id="v-vid" type="number" min="1" max="4094" bind:value={edit.vid}>{#if errs?.field('vid')}<span class="ferr">{errs.field('vid')}</span>{/if}</div>
      <div class="field"><label for="v-name">Nom</label><input class="input" id="v-name" bind:value={edit.name} placeholder="Production 2110">{#if errs?.field('name')}<span class="ferr">{errs.field('name')}</span>{/if}</div>
      <div class="field"><label for="v-cidr">Sous-réseau</label><input class="input mono" id="v-cidr" bind:value={edit.cidr} placeholder="10.10.20.0/24">{#if errs?.field('cidr')}<span class="ferr">{errs.field('cidr')}</span>{/if}</div>
      <div class="field"><label for="v-gw">Passerelle</label><input class="input mono" id="v-gw" bind:value={edit.gateway} placeholder="10.10.20.1">{#if errs?.field('gateway')}<span class="ferr">{errs.field('gateway')}</span>{/if}</div>
      <div class="field"><label for="v-role">Rôle</label><input class="input" id="v-role" bind:value={edit.role} list="v-roles" placeholder="Gestion, média principal, Dante…"><datalist id="v-roles">{#each ['Gestion', 'Média principal', 'Média secondaire', 'Dante', 'Autre'] as x}<option value={x}></option>{/each}</datalist></div>
      <div class="field"><span class="flabel">Couleur</span><div class="sw-row">{#each PALETTE as c}<button type="button" class="swc" class:on={edit.color === c} style="--c:{c}" aria-label="Couleur {c}" onclick={() => (edit!.color = c)}></button>{/each}</div>{#if errs?.field('color')}<span class="ferr">{errs.field('color')}</span>{/if}</div>
    </div>
    {#snippet footer()}<button class="btn" onclick={() => (edit = null)}>Annuler</button><button class="btn primary" onclick={saveVlan}>{edit?.id ? 'Enregistrer' : 'Ajouter'}</button>{/snippet}
  </Dialog>
{/if}

<style>
  .nw-head { padding: 16px 20px; display: flex; gap: 20px; align-items: center; justify-content: space-between; flex-wrap: wrap; margin-bottom: 12px; }
  .nw-stats { display: flex; gap: 28px; flex-wrap: wrap; }
  .nw-stats > div { display: flex; flex-direction: column; }
  .nw-stats b { font-size: 26px; line-height: 1.1; letter-spacing: -.01em; }
  .nw-stats .bad b { color: var(--err); }
  .nw-act { display: flex; gap: 8px; flex-wrap: wrap; }
  .nw-issues { padding: 10px 16px; margin-bottom: 12px; border-color: color-mix(in srgb, var(--err) 40%, var(--line)); }
  .nw-issues summary { cursor: pointer; font-weight: 600; display: flex; gap: 8px; align-items: center; color: var(--err); }
  .nw-issues ul { margin: 8px 0 2px; padding-left: 20px; display: grid; gap: 4px; font-size: 14px; }
  .nw-tabs { margin-bottom: 12px; max-width: 100%; overflow-x: auto; flex-wrap: nowrap; }
  .nw-tabs button { white-space: nowrap; }
  .ct-bar { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; margin-bottom: 10px; }
  .ct-bar select { width: auto; height: 40px; }
  .nw-bulk { padding: 8px 12px; border-radius: 12px; background: var(--surface-2); }
  .nw-bulk select { height: 34px; }
  .ct { padding: 4px 8px 8px; }
  .ct-tbl td { vertical-align: middle; padding-top: 6px; padding-bottom: 6px; }
  .ct-tbl tr.picked td { background: color-mix(in srgb, var(--accent, #4F6BFF) 10%, transparent); }
  .ct-tbl tr.warn td:first-child, .ct-tbl tr.warn td:first-child + td { box-shadow: inset 3px 0 0 var(--err); }
  .ck { width: 32px; }
  .end { display: table-cell; }
  .end b { display: block; }
  .cell { border: 1px solid transparent; background: transparent; border-radius: 8px; padding: 4px 6px; font: inherit; color: inherit; width: 100%; min-width: 70px; }
  .cell:hover:not(:disabled), .cell:focus { border-color: var(--line); background: var(--surface); }
  select.cell { min-width: 90px; }
  .vl { display: flex; align-items: center; gap: 4px; }
  .trunk { display: flex; gap: 4px; flex-wrap: wrap; margin-top: 4px; }
  .tv { display: inline-flex; gap: 3px; align-items: center; font: 600 11.5px var(--font-mono, monospace); padding: 1px 6px; border-radius: 99px; border: 1.5px solid var(--c); }
  .tv input { width: 12px; height: 12px; margin: 0; }
  .chip.sm { font-size: 11px; padding: 1px 6px; margin-left: 6px; border-radius: 99px; background: var(--surface-2); font-weight: 600; }
  .ess { font-size: 12px; font-weight: 600; padding: 2px 8px; border-radius: 99px; background: var(--surface-2); }
  .ess.video { color: #3A7BD5; } .ess.audio { color: #0E8550; } .ess.data { color: #B78400; }
  .vgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .vcard { padding: 14px 16px; border-top: 4px solid var(--c); display: grid; gap: 8px; }
  .vtop { display: flex; gap: 10px; align-items: center; }
  .vid { font-size: 22px; font-weight: 700; color: var(--c); min-width: 48px; }
  .vt { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .vdl { display: grid; grid-template-columns: auto 1fr; gap: 2px 12px; margin: 0; font-size: 14px; }
  .vdl dt { color: var(--fg-2); }
  .vdl dd { margin: 0; }
  .bar { height: 6px; border-radius: 99px; background: var(--surface-2); overflow: hidden; }
  .bar i { display: block; height: 100%; background: var(--c); }
  .sw-row { display: flex; gap: 6px; flex-wrap: wrap; }
  .swc { width: 26px; height: 26px; border-radius: 50%; background: var(--c); border: 2px solid transparent; cursor: pointer; }
  .swc.on { border-color: var(--fg); box-shadow: inset 0 0 0 1.5px var(--bg); }
  .ferr { color: var(--err); font-size: 13px; }
  .nw-map { padding: 12px; overflow: auto; }
  .nw-map svg { display: block; max-width: none; }
  .nd { fill: var(--surface); stroke: var(--line-strong, var(--line)); stroke-width: 1.5; }
  .nd.swn { fill: color-mix(in srgb, var(--fg) 6%, var(--surface)); stroke: var(--fg); }
  .nt { font: 600 13px var(--font); fill: var(--fg); }
  .ns { font: 500 11px var(--font); fill: var(--fg-2); }
  .ns.mono { font-family: var(--font-mono, monospace); }
  .lg { display: inline-flex; gap: 6px; align-items: center; }
  .lg i { width: 18px; height: 4px; border-radius: 2px; display: inline-block; }
  .lg i.dash { background: repeating-linear-gradient(90deg, var(--fg-2) 0 6px, transparent 6px 9px); }
  .empty-card { padding: 28px; text-align: center; }
  @media (max-width: 700px) { .nw-stats { gap: 16px; } .nw-stats b { font-size: 20px; } }
</style>
