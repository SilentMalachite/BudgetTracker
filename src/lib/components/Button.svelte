<script lang="ts">
  type Variant = 'primary' | 'ghost' | 'danger';
  let {
    children,
    onclick,
    type = 'button',
    variant = 'primary',
    disabled = false,
  }: {
    children?: any;
    onclick?: (event: MouseEvent) => void;
    type?: 'button' | 'submit';
    variant?: Variant;
    disabled?: boolean;
  } = $props();
</script>

<button {type} {disabled} class={variant} {onclick}>
  {@render children?.()}
</button>

<style>
  button {
    border: 0;
    border-radius: var(--radius-md);
    padding: var(--space-3) var(--space-4);
    font-weight: 600;
    cursor: pointer;
    transition: filter 0.15s ease, background 0.15s ease;
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  button.primary {
    color: white;
    background: linear-gradient(135deg, var(--accent-grad-start), var(--accent-grad-end));
  }

  button.primary:hover:not(:disabled) {
    filter: brightness(1.05);
  }

  button.ghost {
    background: transparent;
    color: var(--text);
    border: 1px solid var(--border);
  }

  button.ghost:hover:not(:disabled) {
    background: rgba(0, 0, 0, 0.04);
  }

  button.danger {
    background: var(--danger);
    color: white;
  }
</style>
