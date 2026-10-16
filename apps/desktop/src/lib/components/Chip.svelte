<script lang="ts">
  interface Props {
    selected: boolean;
    label: string;
    title?: string;
    onchange: () => void;
  }

  let { selected, label, title = "", onchange }: Props = $props();
</script>

<button type="button" class="chip" class:on={selected} {title} onclick={onchange}>
  {label}
</button>