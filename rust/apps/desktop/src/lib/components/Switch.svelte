<script lang="ts">
  interface Props {
    checked: boolean;
    onchange: (value: boolean) => void;
    title?: string;
    disabled?: boolean;
  }

  let { checked, onchange, title = "", disabled = false }: Props = $props();
</script>

<button
  type="button"
  class="switch"
  class:on={checked}
  {title}
  {disabled}
  role="switch"
  aria-checked={checked}
  onclick={() => onchange(!checked)}
>
  <span class="knob"></span>
</button>