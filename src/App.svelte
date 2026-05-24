<script lang="ts">
  import { onMount } from 'svelte';
  import { getAppInfo, type AppInfo } from './lib/api';

  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      info = await getAppInfo();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  });
</script>

<main>
  <h1>BudgetTracker</h1>
  {#if info}
    <p data-testid="schema-version">Schema version: {info.schema_version}</p>
    <p data-testid="db-path">DB: {info.db_path}</p>
  {:else if error}
    <p data-testid="error">Error: {error}</p>
  {:else}
    <p data-testid="loading">Loading…</p>
  {/if}
</main>

<style>
  main {
    max-width: 720px;
    margin: 4rem auto;
    padding: 2rem;
    background: var(--card-bg);
    border-radius: 16px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.18);
  }
  h1 {
    margin-top: 0;
    background: linear-gradient(135deg, var(--bg-grad-start), var(--bg-grad-end));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
</style>
