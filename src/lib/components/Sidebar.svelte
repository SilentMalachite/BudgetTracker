<script lang="ts">
  const items = [
    { path: '/', label: 'ダッシュボード', icon: '🏠' },
    { path: '/transactions', label: '取引', icon: '💸' },
    { path: '/categories', label: 'カテゴリ', icon: '🏷️' },
    { path: '/budgets', label: '予算', icon: '📊' },
    { path: '/recurring', label: '定期取引', icon: '🔁' },
    { path: '/accounts', label: '口座', icon: '🏦' },
    { path: '/settings', label: '設定', icon: '⚙️' },
  ];

  function isActive(path: string, current: string): boolean {
    if (path === '/') return current === '/';
    return current.startsWith(path);
  }

  let {
    currentPath,
    navigate,
  }: {
    currentPath: string;
    navigate: (path: string) => void;
  } = $props();
</script>

<nav class="sidebar" aria-label="Primary navigation">
  <div class="brand">BudgetTracker</div>
  <ul>
    {#each items as item}
      <li>
        <a
          href={item.path}
          class:active={isActive(item.path, currentPath)}
          data-testid={`nav-${item.path === '/' ? 'dashboard' : item.path.slice(1)}`}
          onclick={(event) => {
            event.preventDefault();
            navigate(item.path);
          }}
        >
          <span class="icon" aria-hidden="true">{item.icon}</span>
          <span>{item.label}</span>
        </a>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .sidebar {
    background: rgba(0, 0, 0, 0.25);
    color: white;
    padding: var(--space-5) var(--space-3);
    backdrop-filter: blur(8px);
  }

  .brand {
    font-weight: 700;
    font-size: 1.2rem;
    margin: 0 var(--space-3) var(--space-5);
    letter-spacing: 0.04em;
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  a {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    color: inherit;
    text-decoration: none;
    opacity: 0.85;
  }

  a:hover {
    opacity: 1;
    background: rgba(255, 255, 255, 0.08);
  }

  a.active {
    opacity: 1;
    background: rgba(255, 255, 255, 0.18);
    font-weight: 600;
  }

  .icon {
    font-size: 1.1rem;
  }
</style>
