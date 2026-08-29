<script lang="ts">
  let { message }: { message: string } = $props();
</script>

<p class="error" role="alert" data-testid="page-error">{message}</p>

<style>
  .error {
    color: var(--danger);
    font-weight: 700;
  }
</style>
