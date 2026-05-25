<script lang="ts">
  let { title, hint }: { title: string; hint?: string } = $props();
</script>

<div class="empty">
  <p class="title">{title}</p>
  {#if hint}<p class="hint">{hint}</p>{/if}
</div>

<style>
  .empty {
    text-align: center;
    padding: var(--space-6);
    color: var(--muted);
  }

  .title {
    font-weight: 600;
    margin: 0 0 var(--space-2);
    color: var(--text);
  }

  .hint {
    margin: 0;
  }
</style>
