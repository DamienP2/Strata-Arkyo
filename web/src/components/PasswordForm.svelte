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
  // Erreurs du serveur rattachées à un champ (sinon message général).
  let currentErr = $state<string | null>(null), nextErr = $state<string | null>(null);
  const mismatch = $derived(confirm.length > 0 && confirm !== next);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (mismatch) return;
    busy = true; error = null; currentErr = null; nextErr = null;
    try {
      session.me = await api.post<Me>('/me/password', { current, new: next });
      ondone();
    } catch (x) {
      if (x instanceof ApiError && (x.field('current') || x.field('new'))) { currentErr = x.field('current') ?? null; nextErr = x.field('new') ?? null; }
      else error = x instanceof ApiError ? x.message : 'Changement impossible.';
    } finally { busy = false; }
  }
</script>

<form onsubmit={submit} class="stack">
  <div class="field"><label for="cur">{currentLabel}</label><input class="input" id="cur" type="password" autocomplete="current-password" bind:value={current} required aria-invalid={currentErr ? 'true' : undefined} aria-describedby={currentErr ? 'cur-err' : undefined}><FieldError message={currentErr} for="cur" /></div>
  <div class="field"><label for="new">Nouveau mot de passe</label><input class="input" id="new" type="password" autocomplete="new-password" bind:value={next} required minlength="8" aria-invalid={nextErr ? 'true' : undefined} aria-describedby={nextErr ? 'new-err' : 'new-help'}>{#if nextErr}<FieldError message={nextErr} for="new" />{:else}<span class="help" id="new-help">8 caractères au moins. Vos autres sessions seront fermées.</span>{/if}</div>
  <div class="field"><label for="new2">Confirmation</label><input class="input" id="new2" type="password" autocomplete="new-password" bind:value={confirm} required aria-invalid={mismatch ? 'true' : undefined} aria-describedby={mismatch ? 'new2-err' : undefined}><FieldError message={mismatch ? 'Les deux mots de passe diffèrent.' : null} for="new2" /></div>
  {#if error}<div class="alert err" role="alert">{error}</div>{/if}
  <button class="btn primary" disabled={busy || mismatch}>{#if busy}<span class="spin" aria-hidden="true"></span>{/if}{submitLabel}</button>
</form>
