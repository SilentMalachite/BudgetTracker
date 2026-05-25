<script lang="ts">
  type Option = { value: string; label: string };
  let {
    label,
    value = $bindable(''),
    options,
    required = false,
    testid,
  }: {
    label: string;
    value?: string;
    options: Option[];
    required?: boolean;
    testid?: string;
  } = $props();
</script>

<label class="field">
  <span>{label}{#if required}<em>*</em>{/if}</span>
  <select bind:value {required} data-testid={testid}>
    {#each options as opt}
      <option value={opt.value}>{opt.label}</option>
    {/each}
  </select>
</label>

<style>
  .field {
    display: grid;
    gap: var(--space-2);
  }

  .field span {
    font-size: 0.85rem;
    color: var(--muted);
  }

  .field em {
    color: var(--danger);
    margin-left: var(--space-1);
    font-style: normal;
  }

  select {
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--surface);
  }
</style>
