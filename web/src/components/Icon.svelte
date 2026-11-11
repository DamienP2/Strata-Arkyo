<script lang="ts">
  import { ICONS } from '../lib/icons';
  let { name, size = '' }: { name: string; size?: '' | 'sm' } = $props();
</script>

<svg class="ic {size}" viewBox="0 0 24 24" aria-hidden="true"><path d={ICONS[name] ?? ''} /></svg>
