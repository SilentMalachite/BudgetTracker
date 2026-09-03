<script lang="ts">
  let { ariaLabel, testId }: { ariaLabel: string; testId?: string } = $props();
</script>

<div aria-label={ariaLabel} data-testid={testId}></div>
