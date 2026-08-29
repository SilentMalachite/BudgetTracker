<script lang="ts">
  import { onMount } from 'svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Dashboard from './routes/Dashboard.svelte';
  import Transactions from './routes/Transactions.svelte';
  import Categories from './routes/Categories.svelte';
  import Budgets from './routes/Budgets.svelte';
  import Accounts from './routes/Accounts.svelte';
  import Settings from './routes/Settings.svelte';
  import Recovery from './routes/Recovery.svelte';
  import { bootStatus, type BootStatus } from './lib/api/boot';

  const routePaths = new Set([
    '/',
    '/transactions',
    '/categories',
    '/budgets',
    '/accounts',
    '/settings',
  ]);

  function normalizePath(path: string): string {
    return routePaths.has(path) ? path : '/';
  }

  let currentPath = $state('/');
  let boot = $state<BootStatus | null>(null);
  let bootError = $state<string | null>(null);

  onMount(() => {
    currentPath = normalizePath(window.location.pathname);
    const handlePop = () => {
      currentPath = normalizePath(window.location.pathname);
    };
    window.addEventListener('popstate', handlePop);
    void bootStatus()
      .then((status) => {
        boot = status;
      })
      .catch((e) => {
        bootError = e instanceof Error ? e.message : String(e);
      });
    return () => window.removeEventListener('popstate', handlePop);
  });

  function navigate(path: string) {
    const next = normalizePath(path);
    if (window.location.pathname !== next) {
      window.history.pushState({}, '', next);
    }
    currentPath = next;
  }
</script>

{#if bootError}
  <div class="boot-message" data-testid="boot-error">{bootError}</div>
{:else if boot === null}
  <div class="boot-message" data-testid="boot-loading">読み込み中</div>
{:else if boot.state === 'recovery'}
  <Recovery reason={boot.recovery_reason} dbPath={boot.db_path} />
{:else}
  <div class="app-shell">
    <Sidebar {currentPath} {navigate} />
    <main class="app-main">
      {#if currentPath === '/transactions'}
        <Transactions />
      {:else if currentPath === '/categories'}
        <Categories />
      {:else if currentPath === '/budgets'}
        <Budgets />
      {:else if currentPath === '/accounts'}
        <Accounts />
      {:else if currentPath === '/settings'}
        <Settings />
      {:else}
        <Dashboard />
      {/if}
    </main>
  </div>
{/if}

<style>
  .boot-message {
    color: white;
    padding: var(--space-6);
  }
</style>
