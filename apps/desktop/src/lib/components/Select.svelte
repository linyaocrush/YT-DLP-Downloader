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
    title?: string;
    disabled?: boolean;
  }

  let { value, options, onchange, width = "", title = "", disabled = false }: Props = $props();
</script>

<div class="select" style={width ? `width:${width}` : ""}>
  <select
    {title}
    {disabled}
    value={value}
    onchange={(event) => onchange(event.currentTarget.value as T)}
  >
    {#each options as option (option.value)}
      <option value={option.value}>{option.label}</option>
    {/each}
  </select>
  <span class="chevron" aria-hidden="true"></span>
</div>