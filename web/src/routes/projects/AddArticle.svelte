<script lang="ts">
  // Ajouter du matériel : deux volets. À gauche, le genre (équipements, câbles, adaptateurs,
  // accessoires) puis les fonctions et catégories du catalogue, avec leurs compteurs ; à droite,
  // la recherche (toujours visible) et les résultats, chacun avec sa quantité et « Ajouter ».
  // En bas, le panier de la session : ce qui vient d'être ajouté, avec « Annuler le dernier ».
  // Clavier : la recherche a le focus, ↑ ↓ parcourent les résultats, Entrée ajoute.
  import { tick, untrack } from 'svelte';
  import Icon from '../../components/Icon.svelte';
  import PanelFace from '../../components/PanelFace.svelte';
  import { blankPanel, PER_U, type PanelData } from '../../lib/panel';
  import { api } from '../../lib/api';
  import { fail } from '../../lib/toast.svelte';
  import { modal } from '../../lib/modal';
  import { loadRefs, refList, refName, refById, fnColor, summary, type CatalogItem, type RefItem } from '../../lib/catalog.svelte';
  import { CONTAINER_KINDS, type Article, type Container, type ProjectView } from '../../lib/projects';

  let { view, container, locationId, onclose, ondone }: { view: ProjectView; container: Container | null; locationId: string; onclose: () => void; ondone: () => void | Promise<void> } = $props();

  type Genre = 'equipment' | 'cable' | 'adapter' | 'accessory';
  type AccKind = 'plate' | 'shelf' | 'drawer' | 'lacing_bar' | 'vertical_pdu' | 'panel';
  const GENRES: { k: Genre; label: string; icon: string }[] = [
    { k: 'equipment', label: 'Équipements', icon: 'server' },
    { k: 'cable', label: 'Câbles', icon: 'cable' },
    { k: 'adapter', label: 'Adaptateurs', icon: 'plug' },
    { k: 'accessory', label: 'Accessoires', icon: 'panel' },
  ];
  const ACC: { k: AccKind; label: string; help: string }[] = [
    { k: 'plate', label: 'Plaque', help: 'Pleine, passe-câble ou aérée' },
    { k: 'shelf', label: 'Étagère', help: 'Pour le matériel hors format rack' },
    { k: 'drawer', label: 'Tiroir', help: 'Rangement coulissant' },
    { k: 'lacing_bar', label: 'Barre de laçage', help: 'Face arrière, pour attacher les câbles' },
    { k: 'vertical_pdu', label: 'PDU vertical', help: 'Dans le couloir arrière, sans U' },
    { k: 'panel', label: 'Panel de brassage', help: 'Depuis la bibliothèque ou vierge' },
  ];

  // ----- Destination : un élément, ou le matériel libre d'un emplacement -----
  // Réglée à l'ouverture (élément de l'appelant), puis libre : on la lit une fois.
  let dest = $state(untrack(() => (container ? `c:${container.id}` : `l:${locationId}`)));
  const destC = $derived(dest.startsWith('c:') ? (view.containers.find((c) => c.id === dest.slice(2)) ?? null) : null);
  const destLoc = $derived(destC ? destC.location_id : dest.slice(2));
  const destLabel = $derived(destC ? destC.name : `${view.locations.find((l) => l.id === destLoc)?.name ?? ''} · matériel libre`);
  const accOk = $derived(!!destC?.rackable);

  // ----- Catalogue : chargé une fois par genre, filtré ici (compteurs instantanés) -----
  let genre = $state<Genre>('equipment');
  let q = $state('');
  let facet = $state<string>(''); // '' | 'fn:<clé>' | 'cat:<id>'
  let pools = $state<Partial<Record<Exclude<Genre, 'accessory'>, CatalogItem[]>>>({});
  let loading = $state(false);
  loadRefs();
  async function loadPool(g: Exclude<Genre, 'accessory'>) {
    if (pools[g]) return;
    loading = true;
    const path = { equipment: 'equipment', cable: 'cables', adapter: 'adapters' }[g];
    try { pools[g] = (await api.get<{ items: CatalogItem[] }>(`/catalog/items?kind=${path}&active=true&limit=1000`)).items; }
    catch (e) { fail(e); pools[g] = []; } finally { loading = false; }
  }
  $effect(() => { if (genre !== 'accessory') loadPool(genre); });
  $effect(() => { if (genre === 'accessory' && !accOk) genre = 'equipment'; });

  const norm = (s: string) => s.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
  /** Fonctions d'une fiche ; à défaut, celle de son type (ou du type parent). */
  function fnsOf(i: CatalogItem): string[] {
    if (i.kind !== 'equipment') return i.data.function ? [i.data.function] : [];
    if (i.data.functions?.length) return i.data.functions;
    const t = refById('types', i.data.type_id);
    const f = t?.data?.function ?? (t?.parent_id ? refById('types', t.parent_id)?.data?.function : null);
    return f ? [f] : [];
  }
  const cats = $derived(refList('categories'));
  const types = $derived(refList('types'));
  /** Nœud et ses ancêtres : une fiche compte à chaque niveau de l'arborescence. */
  function chain(list: RefItem[], id: string | null | undefined): string[] {
    const out: string[] = [];
    let c = id ? list.find((x) => x.id === id) : undefined;
    while (c && !out.includes(c.id)) { out.push(c.id); c = c.parent_id ? list.find((x) => x.id === c!.parent_id) : undefined; }
    return out;
  }
  const hay = (i: CatalogItem) => norm([i.name, i.reference, refName('brands', i.brand_id), refName('types', i.data.type_id), ...fnsOf(i).map((f) => refName('functions', f))].filter(Boolean).join(' '));
  const pool = $derived(genre === 'accessory' ? [] : (pools[genre] ?? []));
  const indexed = $derived(pool.map((i) => ({ i, h: hay(i), fns: fnsOf(i), cats: chain(cats, i.data.category_id), types: chain(types, i.data.type_id) })));
  const words = $derived(norm(q).split(/\s+/).filter(Boolean));
  const matched = $derived(words.length ? indexed.filter((x) => words.every((w) => x.h.includes(w))) : indexed);
  const counts = $derived.by(() => {
    const fn = new Map<string, number>(), cat = new Map<string, number>(), type = new Map<string, number>();
    const inc = (m: Map<string, number>, k: string) => m.set(k, (m.get(k) ?? 0) + 1);
    for (const x of matched) { x.fns.forEach((f) => inc(fn, f)); x.cats.forEach((c) => inc(cat, c)); x.types.forEach((t) => inc(type, t)); }
    return { fn, cat, type };
  });
  const fnList = $derived(refList('functions').filter((f) => counts.fn.has(f.id) || facet === `fn:${f.id}`));
  /** Arborescence à plat (profondeur pour l'indentation), sans les nœuds vides. `openOnly` :
   *  les enfants ne s'affichent que sous le nœud choisi (types : longue liste à deux niveaux). */
  function tree(list: RefItem[], n: Map<string, number>, pre: string, openOnly: boolean) {
    const out: { c: RefItem; depth: number }[] = [];
    const sel = facet.startsWith(pre) ? chain(list, facet.slice(pre.length)) : [];
    const walk = (parent: string | null, depth: number) => {
      for (const c of list.filter((x) => (x.parent_id ?? null) === parent).sort((a, b) => a.position - b.position || a.name.localeCompare(b.name, 'fr'))) {
        if (!n.has(c.id) && !sel.includes(c.id)) continue;
        out.push({ c, depth });
        if (!openOnly || sel.includes(c.id)) walk(c.id, depth + 1);
      }
    };
    walk(null, 0);
    return out;
  }
  const catTree = $derived(tree(cats, counts.cat, 'cat:', false));
  const typeTree = $derived(tree(types, counts.type, 'type:', true));
  const typeTreeAll = $derived(tree(types, counts.type, 'type:', false));
  const hasKids = (id: string) => types.some((t) => t.parent_id === id && counts.type.has(t.id));
  const typeOpen = (id: string) => facet === `type:${id}` || refById('types', facet.slice(5))?.parent_id === id;
  $effect(() => { genre; facet = ''; });
  const results = $derived.by(() => {
    let r = matched;
    if (facet.startsWith('fn:')) r = r.filter((x) => x.fns.includes(facet.slice(3)));
    else if (facet.startsWith('cat:')) r = r.filter((x) => x.cats.includes(facet.slice(4)));
    else if (facet.startsWith('type:')) r = r.filter((x) => x.types.includes(facet.slice(5)));
    const first = words[0];
    return [...r].sort((a, b) => (first ? Number(!norm(a.i.name).startsWith(first)) - Number(!norm(b.i.name).startsWith(first)) : 0) || a.i.name.localeCompare(b.i.name, 'fr', { numeric: true })).map((x) => x.i);
  });
  const MAXROWS = 150;
  const shown = $derived(results.slice(0, MAXROWS));
  const facetLabel = $derived(facet.startsWith('fn:') ? refName('functions', facet.slice(3)) : facet.startsWith('cat:') ? refName('categories', facet.slice(4)) : facet.startsWith('type:') ? refName('types', facet.slice(5)) : '');

  // ----- Clavier -----
  let active = $state(0);
  let searchEl = $state<HTMLInputElement>();
  let listEl = $state<HTMLElement>();
  $effect(() => { results; active = 0; });
  async function reveal() { await tick(); listEl?.querySelector<HTMLElement>(`[data-row="${active}"]`)?.scrollIntoView({ block: 'nearest' }); }
  function onSearchKey(e: KeyboardEvent) {
    if (genre === 'accessory' || genre === 'cable') return;
    if (e.key === 'ArrowDown' && shown.length) { e.preventDefault(); active = Math.min(shown.length - 1, active + 1); reveal(); }
    else if (e.key === 'ArrowUp' && shown.length) { e.preventDefault(); active = Math.max(0, active - 1); reveal(); }
    else if (e.key === 'Enter' && shown[active]) { e.preventDefault(); if (!busy) addItem(shown[active]!); }
  }

  // ----- Ajouts et panier -----
  let qty = $state<Record<string, number>>({});
  const setQ = (id: string, v: number) => (qty[id] = Math.min(999, Math.max(1, Math.round(v) || 1)));
  let busy = $state('');
  interface Line { article: Article; label: string; qty: number; merged: boolean; where: string; note: string }
  let cart = $state<Line[]>([]);
  async function add(body: object, key: string, label: string, n: number) {
    if (busy) return;
    busy = key;
    try {
      const r = await api.post<{ merged: boolean; warnings: string[]; article: Article; modules?: Article[] }>(`/projects/${view.project.id}/articles`, { container_id: destC?.id ?? null, location_id: destLoc, ...body });
      cart.push({ article: r.article, label, qty: n, merged: r.merged, where: destLabel, note: r.warnings.join(' ') });
      await ondone();
    } catch (e) { fail(e); } finally { busy = ''; }
  }
  async function undo() {
    const l = cart.at(-1); if (!l || busy) return;
    busy = 'undo';
    try {
      if (l.merged && l.article.quantity > l.qty) await api.patch(`/articles/${l.article.id}`, { rev: l.article.rev, quantity: l.article.quantity - l.qty });
      else await api.del(`/articles/${l.article.id}`);
      cart.pop();
      await ondone();
    } catch (e) { fail(e); } finally { busy = ''; searchEl?.focus(); }
  }
  const total = $derived(cart.reduce((n, l) => n + l.qty, 0));
  const addItem = (i: CatalogItem) => { const n = qty[i.id] ?? 1; return add({ nature: i.kind === 'adapter' ? 'adapter' : 'equipment', item_id: i.id, quantity: n }, i.id, i.name, n); };

  // Accessoires
  let accKind = $state<AccKind>('plate');
  let acc = $state<{ variant: 'blank' | 'brush' | 'vented'; depth_mm: number; height_u: number; length_mm: number; outlets: number; side: 'left' | 'right' }>({ variant: 'blank', depth_mm: 450, height_u: 1, length_mm: 1600, outlets: 24, side: 'left' });
  let lib = $state<{ id: string; panel: PanelData }[] | null>(null);
  let blankP = $state<PanelData>(blankPanel('Panel'));
  $effect(() => { if (genre === 'accessory' && accKind === 'panel' && !lib) api.get<{ panels: { id: string; panel: PanelData }[] }>('/panels').then((r) => (lib = r.panels), fail); });
  const libShown = $derived((lib ?? []).filter((l) => words.every((w) => norm(l.panel.name).includes(w))));
  const addPanel = (panel: PanelData, key: string) => add({ nature: 'accessory', accessory: { kind: 'panel', panel } }, key, `Panel · ${panel.name}`, 1);
  const PLATE: Record<string, string> = { blank: 'pleine', brush: 'passe-câble', vented: 'aérée' };
  function addAcc() {
    const k = accKind;
    const a = k === 'plate' ? { kind: 'plate', variant: acc.variant, height_u: acc.height_u } : k === 'vertical_pdu' ? { kind: 'vertical_pdu', length_mm: acc.length_mm, outlets: acc.outlets, side: acc.side } : { kind: k, depth_mm: acc.depth_mm, height_u: acc.height_u };
    const name = ACC.find((x) => x.k === k)!.label;
    const label = k === 'plate' ? `${name} ${PLATE[acc.variant]} ${acc.height_u} U` : k === 'vertical_pdu' ? `${name} ${acc.outlets} prises` : k === 'lacing_bar' ? `${name} ${acc.height_u} U` : `${name} ${acc.height_u} U · ${acc.depth_mm} mm`;
    return add({ nature: 'accessory', accessory: a }, 'acc', label, 1);
  }
  function pickAcc(k: AccKind) { accKind = k; if (k === 'lacing_bar') acc.depth_mm = 100; else if (acc.depth_mm === 100) acc.depth_mm = 450; }

  // ----- Téléphone : les catégories passent dans une liste déroulante -----
  let mobileFacet = $derived(genre === 'accessory' ? `acc:${accKind}` : facet);
  function onMobileFacet(v: string) { if (v.startsWith('acc:')) pickAcc(v.slice(4) as AccKind); else facet = v; }
  function pickGenre(g: Genre) { genre = g; searchEl?.focus(); }
  function pickFacet(f: string) { facet = facet === f ? '' : f; searchEl?.focus(); }
  const hText = (i: CatalogItem) => (i.kind === 'equipment' ? (i.data.format?.kind === 'rack' ? `${String(i.data.format.height_u).replace('.', ',')} U` : 'Hors rack') : '');
  const genreCount = (g: Genre) => (g === 'accessory' ? null : pools[g]?.length ?? null);
</script>

<div class="modal addm" role="presentation" use:modal={{ close: onclose, ask: onclose }}>
  <div class="dialog am" role="dialog" aria-modal="true" aria-label="Ajouter du matériel">
    <header class="am-head">
      <div class="am-title"><h2>Ajouter du matériel</h2>
        <label class="am-dest"><span>dans</span>
          <select class="input" bind:value={dest} aria-label="Destination">
            {#each view.locations as l (l.id)}
              <optgroup label={l.name}>
                {#each view.containers.filter((c) => c.location_id === l.id) as c (c.id)}<option value="c:{c.id}">{c.name} · {CONTAINER_KINDS[c.kind].label}</option>{/each}
                <option value="l:{l.id}">{l.name} · matériel libre</option>
              </optgroup>
            {/each}
          </select>
        </label>
      </div>
      <button class="icon-btn sm" onclick={onclose} aria-label="Fermer"><Icon name="x" /></button>
    </header>

    <div class="am-body">
      <!-- Volet gauche : genre, puis fonctions et catégories -->
      <nav class="am-side" aria-label="Filtres du catalogue">
        <div class="genres" role="group" aria-label="Genre">
          {#each GENRES as g (g.k)}
            {@const n = genreCount(g.k)}
            <button class="gen" class:on={genre === g.k} disabled={g.k === 'accessory' && !accOk} title={g.k === 'accessory' && !accOk ? 'Les accessoires se posent dans un élément rackable (rack, flight-case, casquette).' : ''} onclick={() => pickGenre(g.k)}>
              <Icon name={g.icon} size="sm" /><span>{g.label}</span>{#if n !== null}<em>{n}</em>{/if}
            </button>
          {/each}
        </div>

        <div class="facets">
          {#if genre === 'accessory'}
            <span class="f-t">Accessoires</span>
            {#each ACC as a (a.k)}<button class="fac" class:on={accKind === a.k} onclick={() => pickAcc(a.k)}><span class="fl">{a.label}</span></button>{/each}
          {:else}
            <button class="fac" class:on={facet === ''} onclick={() => pickFacet('')}><span class="fl">Tout</span><em>{matched.length}</em></button>
            {#if fnList.length}
              <span class="f-t">Fonctions</span>
              {#each fnList as f (f.id)}<button class="fac" class:on={facet === `fn:${f.id}`} onclick={() => pickFacet(`fn:${f.id}`)}><i class="dot" style="--c:{fnColor(f.id)}"></i><span class="fl">{f.name}</span><em>{counts.fn.get(f.id) ?? 0}</em></button>{/each}
            {/if}
            {#if catTree.length}
              <span class="f-t">Catégories</span>
              {#each catTree as { c, depth } (c.id)}<button class="fac" class:on={facet === `cat:${c.id}`} style="--d:{depth}" onclick={() => pickFacet(`cat:${c.id}`)}><span class="fl">{c.name}</span><em>{counts.cat.get(c.id) ?? 0}</em></button>{/each}
            {/if}
            {#if typeTree.length}
              <span class="f-t">Types</span>
              {#each typeTree as { c, depth } (c.id)}<button class="fac" class:on={facet === `type:${c.id}`} style="--d:{depth}" onclick={() => pickFacet(`type:${c.id}`)}><span class="fl">{c.name}</span>{#if depth === 0 && hasKids(c.id)}<span class="tw" class:open={typeOpen(c.id)}>›</span>{/if}<em>{counts.type.get(c.id) ?? 0}</em></button>{/each}
            {/if}
          {/if}
        </div>

        <!-- Téléphone -->
        <select class="input m-facet" aria-label="Catégorie" value={mobileFacet} onchange={(e) => onMobileFacet((e.target as HTMLSelectElement).value)}>
          {#if genre === 'accessory'}
            {#each ACC as a (a.k)}<option value="acc:{a.k}">{a.label}</option>{/each}
          {:else}
            <option value="">Toutes les fiches ({matched.length})</option>
            {#if fnList.length}<optgroup label="Fonctions">{#each fnList as f (f.id)}<option value="fn:{f.id}">{f.name} ({counts.fn.get(f.id) ?? 0})</option>{/each}</optgroup>{/if}
            {#if catTree.length}<optgroup label="Catégories">{#each catTree as { c, depth } (c.id)}<option value="cat:{c.id}">{'\u00a0\u00a0'.repeat(depth)}{depth ? '└ ' : ''}{c.name} ({counts.cat.get(c.id) ?? 0})</option>{/each}</optgroup>{/if}
            {#if typeTreeAll.length}<optgroup label="Types">{#each typeTreeAll as { c, depth } (c.id)}<option value="type:{c.id}">{depth ? '\u00a0\u00a0└ ' : ''}{c.name} ({counts.type.get(c.id) ?? 0})</option>{/each}</optgroup>{/if}
          {/if}
        </select>
      </nav>

      <!-- Volet droit : recherche et résultats -->
      <section class="am-main">
        <label class="am-search"><Icon name="search" />
          <input class="input" bind:this={searchEl} bind:value={q} onkeydown={onSearchKey} data-autofocus autocomplete="off" spellcheck="false"
            placeholder={genre === 'accessory' ? (accKind === 'panel' ? 'Rechercher un panel de la bibliothèque…' : 'Recherche : choisissez « Panel de brassage » pour la bibliothèque') : 'Nom, marque, référence, fonction…'}
            aria-label="Rechercher dans le catalogue" role="combobox" aria-expanded="true" aria-controls="am-list" aria-activedescendant={genre !== 'accessory' && shown[active] ? `am-r-${shown[active]!.id}` : undefined}>
          {#if q}<button class="clr" onclick={() => { q = ''; searchEl?.focus(); }} aria-label="Effacer la recherche"><Icon name="x" size="sm" /></button>{/if}
        </label>

        {#if genre !== 'accessory'}
          <div class="am-meta">
            <span><b>{results.length}</b> fiche{results.length > 1 ? 's' : ''}{facetLabel ? ` · ${facetLabel}` : ''}{q.trim() ? ` · « ${q.trim()} »` : ''}</span>
            {#if facet}<button class="ilink" onclick={() => pickFacet('')}>Retirer le filtre</button>{/if}
            <span class="kbd-h"><kbd>↑</kbd><kbd>↓</kbd> parcourir · <kbd>Entrée</kbd> ajouter</span>
          </div>
          {#if genre === 'cable'}
            <div class="alert info am-note"><Icon name="info" /><span>Les câbles ne sont pas du matériel d’inventaire : ils se tirent entre deux ports dans la <a class="ilink" href="/projets/{view.project.id}/cables" onclick={onclose}>liste des câbles</a> du projet. Le catalogue est affiché ici pour référence.</span></div>
          {/if}
          <div class="am-list" id="am-list" role="listbox" aria-label="Résultats" bind:this={listEl}>
            {#each shown as i, k (i.id)}
              {@const fns = fnsOf(i)}
              {@const brand = refName('brands', i.brand_id)}
              <div class="res" class:act={k === active} id="am-r-{i.id}" data-row={k} role="option" aria-selected={k === active} tabindex="-1" onpointerenter={() => (active = k)}>
                <span class="r-name"><b>{i.name}</b><span class="r-sub">{#if brand}<span>{brand}</span>{/if}{#if i.reference}<span class="mono">{i.reference}</span>{/if}{#if i.kind !== 'equipment'}<span>{summary(i)}</span>{/if}</span></span>
                <span class="r-u mono">{hText(i)}</span>
                <span class="r-fn">{#each fns.slice(0, 2) as f}<span class="fchip" style="--c:{fnColor(f)}">{refName('functions', f)}</span>{/each}</span>
                {#if genre !== 'cable'}
                  <span class="r-act">
                    <span class="stepper qty" title="Quantité"><button tabindex="-1" onclick={() => setQ(i.id, (qty[i.id] ?? 1) - 1)} aria-label="Moins">−</button><input class="mono" type="number" min="1" max="999" value={qty[i.id] ?? 1} oninput={(e) => setQ(i.id, Number((e.target as HTMLInputElement).value))} onkeydown={(e) => { if (e.key === 'Enter') addItem(i); }} aria-label="Quantité de {i.name}"><button tabindex="-1" onclick={() => setQ(i.id, (qty[i.id] ?? 1) + 1)} aria-label="Plus">+</button></span>
                    <button class="btn sm" class:primary={k === active} onclick={() => { addItem(i); searchEl?.focus(); }} disabled={!!busy}>{#if busy === i.id}<span class="spin"></span>{:else}<Icon name="plus" size="sm" />{/if}<span class="add-l">Ajouter</span></button>
                  </span>
                {/if}
              </div>
            {:else}
              <div class="empty am-empty">
                {#if loading}<span class="spin"></span>
                {:else if q.trim() || facet}Aucune fiche ne correspond. <button class="ilink" onclick={() => { q = ''; facet = ''; searchEl?.focus(); }}>Tout afficher</button>
                {:else}Le catalogue est vide pour ce genre. <a class="ilink" href="/catalogue/{genre === 'equipment' ? 'equipment' : genre === 'cable' ? 'cables' : 'adapters'}/nouveau">Créer une fiche</a>{/if}
              </div>
            {/each}
            {#if results.length > MAXROWS}<p class="muted small am-more">{results.length - MAXROWS} autres fiches : affinez la recherche ou choisissez une catégorie.</p>{/if}
          </div>
        {:else}
          <div class="am-list acc-pane">
            <div class="acc-head"><b>{ACC.find((a) => a.k === accKind)?.label}</b><span class="muted small">{ACC.find((a) => a.k === accKind)?.help}</span></div>
            {#if accKind === 'panel'}
              <p class="muted small">Le panel est recopié dans le projet : vous pourrez le modifier ensuite sans toucher la bibliothèque.</p>
              <div class="plib">
                {#if lib === null}<span class="spin"></span>{:else}
                  {#each libShown as l (l.id)}
                    <div class="plrow">
                      <span class="plface" style="--h:{l.panel.height_u}"><PanelFace panel={l.panel} /></span>
                      <span class="pltxt"><b>{l.panel.name}</b><span class="muted small">{l.panel.height_u} U · {l.panel.slots.length} connecteur{l.panel.slots.length > 1 ? 's' : ''}</span></span>
                      <button class="btn sm primary" onclick={() => addPanel(l.panel, l.id)} disabled={!!busy}>{#if busy === l.id}<span class="spin"></span>{:else}<Icon name="plus" size="sm" />{/if}Ajouter</button>
                    </div>
                  {:else}<p class="muted">{lib.length ? 'Aucun panel ne correspond.' : 'La bibliothèque ne compte aucun panel.'} <a class="ilink" href="/catalogue/panels">Créer un panel</a></p>{/each}
                {/if}
              </div>
              <div class="fg3">
                <div class="field"><label for="bp-n">Panel vierge</label><input class="input" id="bp-n" bind:value={blankP.name} maxlength="100"></div>
                <div class="field"><label for="bp-u">Hauteur</label><select class="input" id="bp-u" bind:value={blankP.height_u}>{#each [1, 2, 3, 4] as u}<option value={u}>{u} U</option>{/each}</select></div>
                <div class="field"><label for="bp-d">Par U</label><select class="input" id="bp-d" bind:value={blankP.per_u}>{#each PER_U as n}<option value={n}>{n}</option>{/each}</select></div>
              </div>
              <div class="row-actions"><button class="btn" onclick={() => addPanel(blankP, 'blank')} disabled={!!busy || !blankP.name.trim()}><Icon name="plus" />Ajouter un panel vierge</button></div>
            {:else}
              {#if accKind === 'vertical_pdu'}
                <div class="fg3">
                  <div class="field"><label for="ac-s">Couloir (vu de l’arrière)</label><select class="input" id="ac-s" bind:value={acc.side}><option value="left">Gauche</option><option value="right">Droit</option></select></div>
                  <div class="field"><label for="ac-l">Longueur</label><select class="input" id="ac-l" bind:value={acc.length_mm}>{#each [600, 900, 1200, 1500, 1600, 1800, 2000] as d}<option value={d}>{d} mm</option>{/each}</select></div>
                  <div class="field"><label for="ac-o">Prises</label><select class="input" id="ac-o" bind:value={acc.outlets}>{#each [8, 12, 16, 20, 24, 30, 36, 42, 48] as d}<option value={d}>{d}</option>{/each}</select></div>
                </div>
                <p class="muted small">Monté à l’arrière, dans le couloir entre le montant et le flanc : il n’occupe aucun U et s’affiche en vue arrière et en 3D.</p>
              {:else}
                <div class="fg3">
                  <div class="field"><label for="ac-u">Hauteur</label><select class="input" id="ac-u" bind:value={acc.height_u}>{#each [1, 2, 3, 4, 5, 6, 7, 8] as u}<option value={u}>{u} U</option>{/each}</select></div>
                  {#if accKind === 'plate'}<div class="field"><label for="ac-v">Variante</label><select class="input" id="ac-v" bind:value={acc.variant}><option value="blank">Pleine</option><option value="brush">Passe-câble</option><option value="vented">Aérée</option></select></div>
                  {:else if accKind === 'lacing_bar'}<div class="field"><label for="ac-d">Déport depuis le rail</label><select class="input" id="ac-d" bind:value={acc.depth_mm}>{#each [0, 50, 75, 100, 150, 200] as d}<option value={d}>{d} mm</option>{/each}</select><span class="help">Se pose en face arrière, derrière les équipements à attacher.</span></div>
                  {:else}<div class="field"><label for="ac-d">Profondeur</label><select class="input" id="ac-d" bind:value={acc.depth_mm}>{#each accKind === 'drawer' ? [200, 250, 300, 350, 400, 450, 500] : [200, 300, 350, 400, 450, 500, 600, 700, 800, 1000, 1200] as d}<option value={d}>{d} mm</option>{/each}</select></div>{/if}
                </div>
              {/if}
              <div class="row-actions"><button class="btn primary" onclick={addAcc} disabled={!!busy}>{#if busy === 'acc'}<span class="spin"></span>{:else}<Icon name="plus" />{/if}Ajouter l’accessoire</button></div>
            {/if}
          </div>
        {/if}
      </section>
    </div>

    <!-- Panier de la session -->
    <footer class="am-cart" aria-live="polite">
      <div class="cart-l">
        {#if cart.length}
          <span class="cart-t"><Icon name="check" size="sm" /><b>{total}</b> ajouté{total > 1 ? 's' : ''}</span>
          <div class="cart-items">
            {#each [...cart].reverse() as l, k (k)}
              <span class="ci" class:last={k === 0} title="{l.label} → {l.where}{l.note ? ` · ${l.note}` : ''}"><b class="mono">{l.qty} ×</b> {l.label}{#if l.note}<i class="warn-d" aria-label="Remarque"></i>{/if}</span>
            {/each}
          </div>
        {:else}
          <span class="muted small">Rien d’ajouté pour l’instant · destination : <b>{destLabel}</b></span>
        {/if}
      </div>
      <div class="cart-r">
        {#if cart.length}<button class="btn ghost" onclick={undo} disabled={!!busy}>{#if busy === 'undo'}<span class="spin"></span>{:else}<Icon name="undo" size="sm" />{/if}Annuler le dernier</button>{/if}
        <button class="btn" class:primary={cart.length > 0} onclick={onclose}>Terminé</button>
      </div>
    </footer>
  </div>
</div>

<style>
  .modal.addm { padding-block: max(4vh, 16px) 16px; }
  .dialog.am { width: min(1180px, 100%); height: min(860px, 92vh); display: grid; grid-template-rows: auto minmax(0, 1fr) auto; }
  .am-head { display: flex; align-items: center; gap: 16px; padding: 18px 22px 14px; border-bottom: 1px solid var(--line); }
  .am-title { flex: 1; min-width: 0; display: flex; align-items: center; gap: 14px; flex-wrap: wrap; }
  .am-title h2 { font: 720 26px/1.1 var(--display); letter-spacing: -.03em; margin: 0; }
  .am-dest { display: flex; align-items: center; gap: 8px; color: var(--fg-3); font-size: 14px; }
  .am-dest select { height: 38px; width: auto; max-width: 420px; font-size: 14.5px; font-weight: 600; }

  .am-body { display: grid; grid-template-columns: 270px minmax(0, 1fr); min-height: 0; }
  .am-side { border-right: 1px solid var(--line); background: var(--surface-2); display: flex; flex-direction: column; min-height: 0; }
  .genres { display: grid; gap: 4px; padding: 14px 12px; border-bottom: 1px solid var(--line); }
  .gen { display: flex; align-items: center; gap: 9px; height: 38px; padding: 0 12px; border: 0; border-radius: 10px; background: var(--surface); color: var(--fg-2); font-size: 13.5px; font-weight: 600; box-shadow: inset 0 0 0 1px var(--line); text-align: left; min-width: 0; }
  .gen span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .gen em { font: 600 11px var(--mono); font-style: normal; color: var(--fg-4); }
  .gen:hover:not(:disabled):not(.on) { color: var(--fg); box-shadow: inset 0 0 0 1px var(--line-2); }
  .gen.on { background: var(--primary-bg); color: var(--primary-fg); box-shadow: var(--key-ink); }
  .gen.on em { color: inherit; opacity: .75; }
  .gen:disabled { opacity: .45; cursor: not-allowed; }
  .facets { flex: 1; overflow: auto; padding: 10px 10px 16px; display: flex; flex-direction: column; gap: 1px; }
  .f-t { font: 600 10.5px var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--fg-3); padding: 14px 10px 6px; }
  .fac { display: flex; align-items: center; gap: 9px; min-height: 34px; padding: 0 10px 0 calc(10px + var(--d, 0) * 16px); border: 0; border-radius: 9px; background: none; color: var(--fg-2); font-size: 14px; text-align: left; }
  .fac:hover { background: var(--surface-3); color: var(--fg); }
  .fac.on { background: var(--accent-soft); color: var(--fg); font-weight: 650; box-shadow: inset 3px 0 0 var(--accent); }
  .fac .fl { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tw { color: var(--fg-4); font-size: 15px; transition: transform .15s; }
  .tw.open { transform: rotate(90deg); }
  .fac em { font: 600 11.5px var(--mono); font-style: normal; color: var(--fg-4); }
  .fac.on em { color: var(--accent); }
  .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--c); flex: none; }
  .m-facet { display: none; }

  .am-main { display: flex; flex-direction: column; min-height: 0; min-width: 0; }
  .am-search { position: relative; display: block; padding: 16px 20px 10px; }
  .am-search > :global(.ic) { position: absolute; left: 36px; top: 31px; color: var(--fg-3); pointer-events: none; }
  .am-search .input { height: 52px; font-size: 17px; padding-left: 48px; padding-right: 44px; border-radius: 14px; }
  .am-search .input:focus { box-shadow: var(--well), 0 0 0 2px var(--accent); }
  .clr { position: absolute; right: 30px; top: 28px; width: 28px; height: 28px; border: 0; border-radius: 8px; background: var(--surface-3); color: var(--fg-2); display: grid; place-items: center; }
  .am-meta { display: flex; align-items: center; gap: 12px; padding: 0 22px 8px; font-size: 13.5px; color: var(--fg-3); flex-wrap: wrap; }
  .am-meta b { color: var(--fg); }
  .kbd-h { margin-left: auto; font-size: 12.5px; }
  kbd { font: 600 11px var(--mono); padding: 1px 5px; border-radius: 5px; background: var(--surface-3); box-shadow: inset 0 -1px 0 var(--line-2); margin-right: 3px; color: var(--fg-2); }
  .am-note { margin: 0 20px 8px; }
  .am-list { flex: 1; overflow: auto; padding: 0 12px 12px; min-height: 0; }
  .res { display: grid; grid-template-columns: minmax(0, 1fr) 76px minmax(0, 180px) auto; align-items: center; gap: 12px; min-height: 58px; padding: 7px 10px; border-radius: 12px; border-bottom: 1px solid var(--line-soft); }
  .res.act { background: var(--accent-soft); border-color: transparent; box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent); }
  .r-name { min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .r-name b { font-size: 15px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .r-sub { display: flex; gap: 10px; font-size: 12.5px; color: var(--fg-3); min-width: 0; overflow: hidden; white-space: nowrap; }
  .r-sub > span { overflow: hidden; text-overflow: ellipsis; }
  .r-u { font-size: 13px; color: var(--fg-2); text-align: right; white-space: nowrap; }
  .r-fn { display: flex; gap: 5px; flex-wrap: wrap; justify-content: flex-start; min-width: 0; }
  .fchip { display: inline-flex; align-items: center; height: 22px; padding: 0 8px; border-radius: 999px; font-size: 12px; font-weight: 600; color: var(--c); background: color-mix(in srgb, var(--c) 14%, transparent); white-space: nowrap; }
  .r-act { display: flex; align-items: center; gap: 8px; }
  .qty { padding: 2px; }
  .qty button { width: 28px; height: 28px; font-size: 16px; }
  .qty input { width: 40px; height: 28px; border: 0; background: none; text-align: center; font-size: 14px; color: var(--fg); -moz-appearance: textfield; appearance: textfield; }
  .qty input::-webkit-inner-spin-button, .qty input::-webkit-outer-spin-button { -webkit-appearance: none; margin: 0; }
  .am-empty { padding: 40px 12px; }
  .am-more { text-align: center; padding: 10px; }

  .acc-pane { padding: 4px 22px 18px; display: flex; flex-direction: column; gap: 14px; }
  .acc-head { display: flex; flex-direction: column; gap: 2px; }
  .acc-head b { font: 680 19px var(--display); }
  .plib { display: grid; gap: 8px; max-height: 40vh; overflow: auto; }
  .plrow { display: flex; gap: 12px; align-items: center; padding: 8px; border-radius: 12px; background: var(--surface-2); }
  .plface { width: 200px; height: calc(var(--h) * 18px); flex: none; background: linear-gradient(180deg, #363B42, #1D2025); border-radius: 4px; padding: 2px 4px; }
  .pltxt { flex: 1; min-width: 0; display: flex; flex-direction: column; }

  .am-cart { display: flex; align-items: center; gap: 14px; padding: 12px 20px; border-top: 1px solid var(--line); background: var(--surface-2); min-height: 64px; }
  .cart-l { flex: 1; min-width: 0; display: flex; align-items: center; gap: 12px; }
  .cart-t { display: inline-flex; align-items: center; gap: 6px; color: var(--ok); white-space: nowrap; font-size: 14px; }
  .cart-items { display: flex; gap: 6px; overflow-x: auto; scrollbar-width: thin; min-width: 0; }
  .ci { display: inline-flex; align-items: center; gap: 5px; height: 30px; padding: 0 11px; border-radius: 999px; background: var(--surface); box-shadow: inset 0 0 0 1px var(--line); font-size: 13px; white-space: nowrap; color: var(--fg-2); }
  .ci.last { box-shadow: inset 0 0 0 1.5px var(--ok); color: var(--fg); animation: pop .24s var(--ease); }
  .warn-d { width: 7px; height: 7px; border-radius: 50%; background: var(--warn); }
  .cart-r { display: flex; gap: 8px; flex: none; }

  @media (max-width: 760px) {
    .modal.addm { padding: 0; align-items: stretch; }
    .dialog.am { height: 100vh; height: 100dvh; border-radius: 0; }
    .am-head { padding: calc(12px + env(safe-area-inset-top, 0px)) 16px 10px; align-items: flex-start; }
    .am-title { gap: 8px; }
    .am-title h2 { font-size: 21px; }
    .am-dest { width: 100%; }
    .am-dest select { flex: 1; max-width: none; }
    .am-body { grid-template-columns: minmax(0, 1fr); grid-template-rows: auto minmax(0, 1fr); }
    .am-side { border-right: 0; border-bottom: 1px solid var(--line); padding: 10px 16px; gap: 8px; }
    .genres { display: flex; overflow-x: auto; padding: 0; border: 0; scrollbar-width: none; }
    .gen { flex: none; height: 36px; }
    .facets { display: none; }
    .m-facet { display: block; height: 40px; font-size: 15px; }
    .am-search { padding: 10px 16px 8px; }
    .am-search > :global(.ic) { left: 30px; top: 23px; }
    .clr { right: 24px; top: 20px; }
    .am-search .input { height: 48px; font-size: 16px; }
    .am-meta { padding: 0 16px 6px; }
    .am-note { margin: 0 16px 8px; }
    .am-list { padding: 0 8px 10px; }
    .res { grid-template-columns: minmax(0, 1fr) auto; grid-template-areas: 'n a' 'f a'; row-gap: 4px; column-gap: 10px; }
    .r-name { grid-area: n; }
    .r-u { display: none; }
    .r-fn { grid-area: f; }
    .r-act { grid-area: a; gap: 6px; }
    .r-act .btn { min-width: 40px; min-height: 40px; padding: 0 10px; }
    .add-l, .kbd-h { display: none; }
    .acc-pane { padding: 4px 16px 16px; }
    .plface { width: 110px; }
    .am-cart { padding: 10px 16px calc(10px + env(safe-area-inset-bottom, 0px)); flex-wrap: wrap; gap: 8px; }
    .cart-l { flex-basis: 100%; }
    .cart-r { flex: 1; justify-content: flex-end; }
  }
</style>
