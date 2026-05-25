<script lang="ts">
  let {
    label,
    value = $bindable(''),
    required = false,
    testid,
  }: {
    label: string;
    value?: string;
    required?: boolean;
    testid?: string;
  } = $props();
</script>

<label class="field">
  <span>{label}{#if required}<em>*</em>{/if}</span>
  <input type="date" bind:value {required} data-testid={testid} />
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

  input {
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--surface);
  }
</style>
