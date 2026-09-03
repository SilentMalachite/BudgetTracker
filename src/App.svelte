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
  import Recurring from './routes/Recurring.svelte';
  import { bootStatus, type BootStatus } from './lib/api/boot';
  import { recurringExpansion } from './lib/stores/recurringExpansion.svelte';

  const routePaths = new Set([
    '/',
    '/transactions',
    '/categories',
    '/budgets',
    '/recurring',
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
        if (status.state !== 'ready') return;
        // run() は失敗を throw せずストアの error に残す。アプリは開いたまま、
        // 失敗はバナーに出て、Recurring 画面の再試行ボタンから同じ展開を呼び直せる。
        return recurringExpansion.run();
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
      {#if recurringExpansion.error}
        <div class="banner banner-error" data-testid="recurring-expansion-error-banner">
          定期取引の展開に失敗しました: {recurringExpansion.error}
          <a
            href="/recurring"
            onclick={(event) => {
              event.preventDefault();
              navigate('/recurring');
            }}>定期取引を開く</a
          >
        </div>
      {/if}
      {#if recurringExpansion.result && recurringExpansion.result.generated > 0}
        <div class="banner banner-info" data-testid="recurring-expansion-banner">
          定期取引を {recurringExpansion.result.generated} 件生成しました
        </div>
      {/if}
      {#if recurringExpansion.result && recurringExpansion.result.skipped.length > 0}
        <div class="banner banner-warn" data-testid="recurring-skip-banner">
          {recurringExpansion.result.skipped.length} 件のルールを見送りました。参照先の口座・カテゴリを確認してください
          <a
            href="/recurring"
            onclick={(event) => {
              event.preventDefault();
              navigate('/recurring');
            }}>定期取引を開く</a
          >
        </div>
      {/if}
      {#if currentPath === '/transactions'}
        <Transactions />
      {:else if currentPath === '/categories'}
        <Categories />
      {:else if currentPath === '/budgets'}
        <Budgets />
      {:else if currentPath === '/recurring'}
        <Recurring />
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

  .banner {
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    margin-bottom: var(--space-4);
  }

  .banner-info {
    background: rgba(79, 172, 254, 0.18);
  }

  .banner-warn {
    background: rgba(255, 170, 0, 0.22);
  }

  .banner-error {
    background: rgba(255, 71, 87, 0.22);
  }

  .banner a {
    margin-left: var(--space-3);
  }
</style>
