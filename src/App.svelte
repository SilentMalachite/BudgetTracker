<script lang="ts">
  import { onMount } from 'svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Dashboard from './routes/Dashboard.svelte';
  import Transactions from './routes/Transactions.svelte';
  import Categories from './routes/Categories.svelte';
  import Budgets from './routes/Budgets.svelte';
  import Accounts from './routes/Accounts.svelte';
  import Settings from './routes/Settings.svelte';

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

  onMount(() => {
    currentPath = normalizePath(window.location.pathname);
    const handlePop = () => {
      currentPath = normalizePath(window.location.pathname);
    };
    window.addEventListener('popstate', handlePop);
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
