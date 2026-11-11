<script lang="ts">
  import { initials } from '../lib/format';
  let { name, color = '#5E626A', size = '' }: { name: string; color?: string; size?: '' | 'sm' | 'lg' } = $props();
</script>

<span class="av {size}" style="--c:{color}" title={name}>{initials(name)}</span>
