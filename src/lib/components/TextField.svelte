<script lang="ts">
  let {
    label,
    value = $bindable(''),
    type = 'text',
    placeholder = '',
    required = false,
    error = null,
    testid,
  }: {
    label: string;
    value?: string;
    type?: 'text' | 'number' | 'date';
    placeholder?: string;
    required?: boolean;
    error?: string | null;
    testid?: string;
  } = $props();
</script>

<label class="field">
  <span>{label}{#if required}<em>*</em>{/if}</span>
  <input
    {type}
    {placeholder}
    bind:value
    {required}
    data-testid={testid}
    aria-invalid={error ? 'true' : undefined}
  />
  {#if error}<small class="error">{error}</small>{/if}
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

  input[aria-invalid='true'] {
    border-color: var(--danger);
  }

  .error {
    color: var(--danger);
    font-size: 0.8rem;
  }
</style>
