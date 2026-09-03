<script lang="ts">
  import type { Category } from '../api/categories';
  import { isHexColor } from '../utils/isHexColor';

  let { category }: { category: Category } = $props();
  const fallback = $derived(category.type === 'income' ? '#4FACFE' : '#FF7A85');
  // Only a strict #RRGGBB reaches the inline style; anything else (e.g. a value that
  // bypassed Rust validation through backup import) falls back to the default colour.
  const bg = $derived(isHexColor(category.color) ? category.color : fallback);
</script>

<span class="badge" style="background:{bg}">
  {#if category.icon}<span class="icon" aria-hidden="true">{category.icon}</span>{/if}
  <span>{category.name}</span>
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 2px 10px;
    border-radius: 999px;
    color: white;
    font-size: 0.85rem;
    font-weight: 600;
  }
</style>
