<script lang="ts">
  import { STATUS, type Status } from '../lib/projects';
  let { status }: { status: Status } = $props();
</script>

<span class="chip st" style="--c:{STATUS[status].color}"><i class="dot"></i>{STATUS[status].label}</span>
