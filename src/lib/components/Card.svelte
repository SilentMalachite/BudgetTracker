<script lang="ts">
  let { children, padded = true }: { children?: any; padded?: boolean } = $props();
</script>

<section class="card" class:padded>
  {@render children?.()}
</section>

<style>
  .card {
    background: var(--card-bg);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-md);
  }

  .card.padded {
    padding: var(--space-5);
  }
</style>
