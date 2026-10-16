<script lang="ts">
  interface Props {
    checked: boolean;
    label: string;
    onchange: () => void;
  }

  let { checked, label, onchange }: Props = $props();
</script>

<button
  type="button"
  class="radio"
  role="radio"
  aria-checked={checked}
  onclick={onchange}
>
  <span class="radio-dot" class:on={checked}></span>
  <span>{label}</span>
</button>