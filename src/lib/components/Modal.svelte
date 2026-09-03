<script lang="ts">
  import type { Snippet } from 'svelte';

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
    children?: Snippet;
    footer?: Snippet;
  } = $props();

  let dialogEl = $state<HTMLDivElement | null>(null);
  let lastFocus: HTMLElement | null = null;

  function focusable(root: HTMLElement): HTMLElement[] {
    return Array.from(
      root.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
      ),
    ).filter((el) => !el.hasAttribute('disabled'));
  }

  function handleBackdrop(event: MouseEvent) {
    if (event.target === event.currentTarget) onclose?.();
  }

  $effect(() => {
    if (!open) return;
    lastFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const frame = requestAnimationFrame(() => {
      const nodes = dialogEl ? focusable(dialogEl) : [];
      nodes[0]?.focus();
    });
    function onKey(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        event.preventDefault();
        onclose?.();
        return;
      }
      if (event.key !== 'Tab' || !dialogEl) return;
      const nodes = focusable(dialogEl);
      if (nodes.length === 0) return;
      const first = nodes[0];
      const last = nodes[nodes.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
    document.addEventListener('keydown', onKey);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener('keydown', onKey);
      lastFocus?.focus();
    };
  });
</script>

{#if open}
  <div class="backdrop" role="presentation" onclick={handleBackdrop}>
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      bind:this={dialogEl}
    >
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
    max-height: calc(100vh - var(--space-6));
    display: flex;
    flex-direction: column;
  }

  header {
    padding: var(--space-5);
    border-bottom: 1px solid var(--border);
    flex: none;
  }

  h2 {
    margin: 0;
    font-size: 1.2rem;
  }

  .body {
    padding: var(--space-5);
    display: grid;
    gap: var(--space-4);
    overflow-y: auto;
    flex: 1 1 auto;
    min-height: 0;
  }

  footer {
    padding: var(--space-4) var(--space-5);
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
    flex: none;
  }
</style>
