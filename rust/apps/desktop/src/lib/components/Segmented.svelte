<script lang="ts" generics="T extends string">
  interface Option {
    value: T;
    label: string;
  }

  interface Props {
    value: T;
    options: Option[];
    onchange: (value: T) => void;
    width?: string;
  }

  let { value, options, onchange, width = "100%" }: Props = $props();

  const index = $derived(Math.max(0, options.findIndex((option) => option.value === value)));
  const count = $derived(Math.max(1, options.length));
</script>

<div class="segmented" style={`width:${width}; --seg-count:${count}; --seg-index:${index};`}>
  <span class="segmented-thumb"></span>
  {#each options as option (option.value)}
    <button
      type="button"
      class="segment"
      class:on={option.value === value}
      onclick={() => onchange(option.value)}
    >
      {option.label}
    </button>
  {/each}
</div>