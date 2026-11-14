<script lang="ts">
  import { ask } from '../../lib/ask.svelte';
  // Panel posé dans un projet : copie propre au projet, modifiée sans toucher la bibliothèque.
  import { untrack } from 'svelte';
  import Dialog from '../../components/Dialog.svelte';
  import Icon from '../../components/Icon.svelte';
  import PanelEditor from '../../components/PanelEditor.svelte';
  import { api, ApiError } from '../../lib/api';
  import { can } from '../../lib/session.svelte';
  import { toast, fail } from '../../lib/toast.svelte';
  import type { PanelData } from '../../lib/panel';
  import type { Article } from '../../lib/projects';

  let { article, readonly = false, eyebrow = '', onclose, ondone }: { article: Article; readonly?: boolean; eyebrow?: string; onclose: () => void; ondone?: () => void } = $props();
  let rev = $state(untrack(() => article.rev));
  const panel = $derived(article.accessory?.kind === 'panel' ? article.accessory.panel : null);
  // Dernière version enregistrée : c'est elle qui part dans la bibliothèque.
  let current = $state<PanelData | null>(untrack(() => panel));
  let editor = $state<{ isDirty: () => boolean } | null>(null);

  async function save(p: PanelData) {
    const r = await api.put<{ article: Article }>(`/articles/${article.id}/panel`, { rev, panel: p });
    rev = r.article.rev;
    toast('Panel enregistré.');
    ondone?.();
    current = r.article.accessory?.kind === 'panel' ? r.article.accessory.panel : p;
    return current;
  }
  async function toLibrary() {
    if (!current) return;
    try {
      await api.post('/panels', { panel: current });
      toast(`« ${current.name} » ajouté à la bibliothèque.`);
    } catch (e) {
      if (e instanceof ApiError && e.fields[0]?.field === 'name') toast('Un panel de la bibliothèque porte déjà ce nom : renommez celui-ci d’abord.', 'warn');
      else fail(e);
    }
  }
  async function close() {
    if (editor?.isDirty() && !(await ask({ title: 'Fermer sans enregistrer ?', text: 'Les modifications du panel seront perdues.', confirmLabel: 'Fermer sans enregistrer', danger: true }))) return;
    onclose();
  }
</script>

<Dialog title={panel?.name ?? 'Panel'} {eyebrow} onclose={close} xl>
  {#if panel}
    <PanelEditor bind:this={editor} initial={panel} {readonly} onsave={save}>
      {#snippet actions()}{#if can('catalogue', 'ecriture')}<button class="btn ghost" onclick={toLibrary} title="Copier ce panel dans la bibliothèque du site"><Icon name="copy" size="sm" />Ajouter à la bibliothèque</button>{/if}{/snippet}
    </PanelEditor>
  {/if}
</Dialog>
