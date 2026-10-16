<script lang="ts">
  interface Props {
    value: number;
  }

  let { value }: Props = $props();

  const clamped = $derived(Math.min(100, Math.max(0, value)));
</script>

<div class="progress">
  <div class="progress-fill" style={`width:${clamped}%`}></div>
</div>