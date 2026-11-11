<script lang="ts">
  // Changement de son propre mot de passe : l'ancien, puis deux fois le nouveau.
  import FieldError from './FieldError.svelte';
  import { api, ApiError } from '../lib/api';
  import { session } from '../lib/session.svelte';
  import type { Me } from '../lib/types';

  let { ondone, submitLabel = 'Enregistrer', currentLabel = 'Mot de passe actuel' }:
    { ondone: () => void; submitLabel?: string; currentLabel?: string } = $props();
  let current = $state(''), next = $state(''), confirm = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);
  const mismatch = $derived(confirm.length > 0 && confirm !== next);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (mismatch) return;
    busy = true; error = null;
    try {
      session.me = await api.post<Me>('/me/password', { current, new: next });
      ondone();
    } catch (x) {
      error = x instanceof ApiError ? x.message : 'Changement impossible.';
    } finally { busy = false; }
  }
</script>

<form onsubmit={submit} class="stack">
  <div class="field"><label for="cur">{currentLabel}</label><input class="input" id="cur" type="password" autocomplete="current-password" bind:value={current} required></div>
  <div class="field"><label for="new">Nouveau mot de passe</label><input class="input" id="new" type="password" autocomplete="new-password" bind:value={next} required minlength="8"><span class="help">8 caractères au moins. Vos autres sessions seront fermées.</span></div>
  <div class="field"><label for="new2">Confirmation</label><input class="input" id="new2" type="password" autocomplete="new-password" bind:value={confirm} required><FieldError message={mismatch ? 'Les deux mots de passe diffèrent.' : null} /></div>
  {#if error}<div class="alert err" role="alert">{error}</div>{/if}
  <button class="btn primary" disabled={busy || mismatch}>{#if busy}<span class="spin"></span>{/if}{submitLabel}</button>
</form>
