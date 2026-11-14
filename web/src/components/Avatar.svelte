<script lang="ts">
  import { initials } from '../lib/format';
  import { avatarColor } from '../lib/color';
  let { name, color = '#5E626A', size = '' }: { name: string; color?: string; size?: '' | 'sm' | 'lg' } = $props();
</script>

<span class="av {size}" style="--c:{avatarColor(color)}" title={name}>{initials(name)}</span>
