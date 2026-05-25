<script lang="ts">
  let {
    open = false,
    title,
    onclose,
    children,
    footer,
  }: {
    open?: boolean;
    title: string;
    onclose?: () => void;
    children?: any;
    footer?: any;
  } = $props();

  function handleBackdrop(event: MouseEvent) {
    if (event.target === event.currentTarget) onclose?.();
  }
</script>

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={handleBackdrop}
    onkeydown={(event) => event.key === 'Escape' && onclose?.()}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-label={title}>
      <header><h2>{title}</h2></header>
      <div class="body">{@render children?.()}</div>
      {#if footer}<footer>{@render footer()}</footer>{/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: grid;
    place-items: center;
    z-index: 100;
  }

  .dialog {
    background: var(--surface);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    min-width: 360px;
    max-width: min(560px, calc(100vw - var(--space-6)));
  }

  header {
    padding: var(--space-5);
    border-bottom: 1px solid var(--border);
  }

  h2 {
    margin: 0;
    font-size: 1.2rem;
  }

  .body {
    padding: var(--space-5);
    display: grid;
    gap: var(--space-4);
  }

  footer {
    padding: var(--space-4) var(--space-5);
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
  }

</style>
