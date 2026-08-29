<script lang="ts">
  import { onDestroy } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import Card from '../lib/components/Card.svelte';
  import CategoryBadge from '../lib/components/CategoryBadge.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import ErrorBanner from '../lib/components/ErrorBanner.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import TextField from '../lib/components/TextField.svelte';
  import { setBudget, type BudgetStatus } from '../lib/api/budgets';
  import type { Category } from '../lib/api/categories';
  import { createBudgetsStore } from '../lib/stores/budgets.svelte';
  import { formatCurrency } from '../lib/utils/formatCurrency';
  import { isoToday } from '../lib/utils/yearMonth';

  const initialMonth = isoToday().slice(0, 7);
  let selectedMonth = $state(initialMonth);
  const store = createBudgetsStore(initialMonth);
  onDestroy(() => {
    void store.dispose();
  });

  let modalOpen = $state(false);
  let editing = $state<BudgetStatus | null>(null);
  let amount = $state('');
  let threshold = $state('80');
  let formError = $state<string | null>(null);

  function shiftMonth(delta: number) {
    const [year, month] = selectedMonth.split('-').map((part) => Number.parseInt(part, 10));
    const next = new Date(year, month - 1 + delta, 1);
    selectedMonth = `${next.getFullYear()}-${String(next.getMonth() + 1).padStart(2, '0')}`;
    store.setYearMonth(selectedMonth);
  }

  function asCategory(status: BudgetStatus): Category {
    return {
      id: status.category_id,
      name: status.category_name,
      type: 'expense',
      color: status.category_color,
      icon: status.category_icon,
      display_order: 0,
      archived_at: null,
    };
  }

  function openEdit(status: BudgetStatus) {
    editing = status;
    amount = String(status.budgeted);
    threshold = String(status.alert_threshold);
    formError = null;
    modalOpen = true;
  }

  function parseInteger(raw: string | number, min: number, max: number): number | null {
    const trimmed = String(raw).trim();
    if (!/^\d+$/.test(trimmed)) return null;
    const value = Number.parseInt(trimmed, 10);
    return value >= min && value <= max ? value : null;
  }

  async function submit() {
    if (!editing) return;
    formError = null;
    const parsedAmount = parseInteger(amount, 0, Number.MAX_SAFE_INTEGER);
    if (parsedAmount == null) {
      formError = '予算額は0以上の整数で入力してください';
      return;
    }
    const parsedThreshold = parseInteger(threshold, 0, 200);
    if (parsedThreshold == null) {
      formError = '警告閾値は0〜200の整数で入力してください';
      return;
    }

    try {
      await setBudget({
        category_id: editing.category_id,
        year_month: selectedMonth,
        amount: parsedAmount,
        alert_threshold: parsedThreshold,
      });
      await store.load();
      modalOpen = false;
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<section data-testid="page-budgets">
  <header class="page-header">
    <h1>予算</h1>
    <div class="month-controls">
      <Button variant="ghost" onclick={() => shiftMonth(-1)}>
        {#snippet children()}前月{/snippet}
      </Button>
      <label>
        <span>対象月</span>
        <input
          type="month"
          bind:value={selectedMonth}
          data-testid="budget-month"
          onchange={() => store.setYearMonth(selectedMonth)}
        />
      </label>
      <Button variant="ghost" onclick={() => shiftMonth(1)}>
        {#snippet children()}翌月{/snippet}
      </Button>
    </div>
  </header>

  <Card>
    {#snippet children()}
      {#if store.loading && store.items.length === 0}
        <p>読み込み中...</p>
      {:else if store.error}
        <ErrorBanner message={store.error} />
      {:else if store.items.length === 0}
        <EmptyState title="支出カテゴリがありません" hint="カテゴリ画面で支出カテゴリを追加してください" />
      {:else}
        <ul class="budget-list">
          {#each store.items as status (status.category_id)}
            <li class="budget-row" data-testid={`budget-row-${status.category_id}`}>
              <div class="row-main">
                <CategoryBadge category={asCategory(status)} />
                <div class="amounts">
                  <strong>{formatCurrency(status.spent)} / {formatCurrency(status.budgeted)}</strong>
                  <span>{status.budgeted > 0 ? `${status.percent}%` : '未設定'}</span>
                </div>
              </div>

              <div class="progress-wrap">
                <div class="progress" data-testid={`budget-progress-${status.category_id}`}>
                  <span style={`width: ${status.progress_percent}%`}></span>
                </div>
                <small>残り{status.days_left}日 / 予測 {formatCurrency(status.projected)}</small>
              </div>

              <div class="badges">
                {#if status.threshold_reached}
                  <span class="badge warning" data-testid={`budget-alert-${status.category_id}`}>警告</span>
                {/if}
                {#if status.projected_over_budget}
                  <span class="badge danger">予測超過</span>
                {/if}
              </div>

              <Button
                variant="ghost"
                onclick={() => openEdit(status)}
                testid={`budget-edit-${status.category_id}`}
              >
                {#snippet children()}編集{/snippet}
              </Button>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </Card>
</section>

<Modal
  open={modalOpen}
  title={editing ? `${editing.category_name}の予算` : '予算を設定'}
  onclose={() => (modalOpen = false)}
>
  {#snippet children()}
    <TextField label="予算額" required type="number" bind:value={amount} testid="budget-amount" />
    <TextField
      label="警告閾値 (%)"
      required
      type="number"
      bind:value={threshold}
      testid="budget-threshold"
    />
    {#if formError}<small class="error">{formError}</small>{/if}
  {/snippet}
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (modalOpen = false)}>
      {#snippet children()}キャンセル{/snippet}
    </Button>
    <Button onclick={() => void submit()}>
      {#snippet children()}保存{/snippet}
    </Button>
  {/snippet}
</Modal>

<style>
  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-4);
    margin-bottom: var(--space-5);
  }

  h1 {
    color: white;
    margin: 0;
  }

  .month-controls {
    display: flex;
    align-items: end;
    gap: var(--space-3);
  }

  label {
    display: grid;
    gap: var(--space-1);
    color: white;
    font-size: 0.85rem;
  }

  input[type='month'] {
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }

  .budget-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--space-3);
  }

  .budget-row {
    display: grid;
    grid-template-columns: minmax(190px, 1fr) minmax(220px, 1.3fr) auto auto;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: rgba(0, 0, 0, 0.02);
  }

  .row-main,
  .amounts,
  .progress-wrap,
  .badges {
    display: grid;
    gap: var(--space-2);
  }

  .amounts span,
  .progress-wrap small {
    color: var(--muted);
    font-size: 0.85rem;
  }

  .progress {
    height: 10px;
    overflow: hidden;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.08);
  }

  .progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(135deg, var(--accent-grad-start), var(--accent-grad-end));
  }

  .badge {
    display: inline-flex;
    justify-content: center;
    min-width: 64px;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    font-weight: 700;
  }

  .warning {
    color: #7a4a00;
    background: #fff1c2;
  }

  .danger {
    color: #8a1f1f;
    background: #ffd7d7;
  }

  .error {
    color: var(--danger);
  }

  @media (max-width: 860px) {
    .page-header,
    .month-controls {
      align-items: stretch;
      flex-direction: column;
    }

    .budget-row {
      grid-template-columns: 1fr;
    }
  }
</style>
