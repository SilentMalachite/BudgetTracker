<script lang="ts">
  import { onDestroy } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import Card from '../lib/components/Card.svelte';
  import DatePicker from '../lib/components/DatePicker.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import Select from '../lib/components/Select.svelte';
  import TextField from '../lib/components/TextField.svelte';
  import { createAccountsStore } from '../lib/stores/accounts.svelte';
  import { createCategoriesStore } from '../lib/stores/categories.svelte';
  import { createTransactionsStore } from '../lib/stores/transactions.svelte';
  import {
    createTransaction,
    createTransfer,
    deleteTransaction,
    updateTransaction,
    updateTransfer,
    type Transaction,
    type TxType,
  } from '../lib/api/transactions';
  import { isoToday } from '../lib/utils/yearMonth';

  const txStore = createTransactionsStore({}, 50);
  const catStore = createCategoriesStore({ include_archived: false });
  const accStore = createAccountsStore(false);

  onDestroy(() => {
    void txStore.dispose();
    void catStore.dispose();
    void accStore.dispose();
  });

  const yen = new Intl.NumberFormat('ja-JP', { style: 'currency', currency: 'JPY' });

  type FormType = 'income' | 'expense' | 'transfer';

  let filterType = $state<'all' | 'income' | 'expense' | 'transfer'>('all');
  let filterFrom = $state('');
  let filterTo = $state('');
  let filterSearch = $state('');

  function applyFilter() {
    txStore.setFilter({
      type: filterType === 'all' ? undefined : (filterType as TxType),
      from: filterFrom || undefined,
      to: filterTo || undefined,
      search: filterSearch || undefined,
    });
  }

  let modalOpen = $state(false);
  let editing = $state<Transaction | null>(null);
  let formType = $state<FormType>('expense');
  let formDate = $state(isoToday());
  let formAmount = $state('');
  let formAccount = $state('');         // source (and the only account field for income/expense)
  let formCounterAccount = $state('');  // destination, only used when formType === 'transfer'
  let formCategory = $state('');
  let formDescription = $state('');
  let formError = $state<string | null>(null);

  function firstCategoryFor(type: 'income' | 'expense'): string {
    const category = catStore.items.find((item) => item.type === type && !item.archived_at);
    return category ? String(category.id) : '';
  }

  function firstAccount(): string {
    const account = accStore.items.find((item) => !item.archived_at);
    return account ? String(account.id) : '';
  }

  function secondAccount(): string {
    const accounts = accStore.items.filter((item) => !item.archived_at);
    return accounts[1] ? String(accounts[1].id) : '';
  }

  function openCreate() {
    editing = null;
    formType = 'expense';
    formDate = isoToday();
    formAmount = '';
    formAccount = firstAccount();
    formCounterAccount = secondAccount();
    formCategory = firstCategoryFor('expense');
    formDescription = '';
    formError = null;
    modalOpen = true;
  }

  function openEdit(transaction: Transaction) {
    editing = transaction;
    formType = transaction.type;
    formDate = transaction.occurred_on;
    formAmount = String(transaction.amount);
    formAccount = String(transaction.account_id);
    formCounterAccount =
      transaction.counter_account_id != null ? String(transaction.counter_account_id) : secondAccount();
    formCategory = transaction.category_id != null ? String(transaction.category_id) : firstCategoryFor('expense');
    formDescription = transaction.description;
    formError = null;
    modalOpen = true;
  }

  function parsePositiveInteger(raw: string | number): number | null {
    const trimmed = String(raw).trim();
    if (!/^\d+$/.test(trimmed)) return null;
    const value = Number.parseInt(trimmed, 10);
    return value > 0 ? value : null;
  }

  async function submit() {
    formError = null;
    const amount = parsePositiveInteger(formAmount);
    if (amount == null) {
      formError = '金額は正の整数を入力してください';
      return;
    }
    const accountId = parsePositiveInteger(formAccount);
    if (accountId == null) {
      formError = formType === 'transfer' ? '振替元口座を選択してください' : '口座を選択してください';
      return;
    }

    try {
      if (formType === 'transfer') {
        const counterId = parsePositiveInteger(formCounterAccount);
        if (counterId == null) {
          formError = '振替先口座を選択してください';
          return;
        }
        if (counterId === accountId) {
          formError = '振替元と振替先は別の口座を選んでください';
          return;
        }
        const payload = {
          occurred_on: formDate,
          amount,
          account_id: accountId,
          counter_account_id: counterId,
          description: formDescription,
        };
        if (editing) {
          if (editing.type !== 'transfer') {
            formError = '種別の変更はできません。一度削除してから再登録してください';
            return;
          }
          await updateTransfer(editing.id, payload);
        } else {
          await createTransfer(payload);
        }
      } else {
        const categoryId = parsePositiveInteger(formCategory);
        if (categoryId == null) {
          formError = 'カテゴリを選択してください';
          return;
        }
        const payload = {
          occurred_on: formDate,
          type: formType,
          amount,
          account_id: accountId,
          category_id: categoryId,
          description: formDescription,
        };
        if (editing) {
          if (editing.type === 'transfer') {
            formError = '種別の変更はできません。一度削除してから再登録してください';
            return;
          }
          await updateTransaction(editing.id, payload);
        } else {
          await createTransaction(payload);
        }
      }
      modalOpen = false;
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    }
  }

  async function remove(transaction: Transaction) {
    if (!confirm(`「${transaction.description || '取引'}」を削除しますか?`)) return;
    await deleteTransaction(transaction.id);
  }

  const categoryById = $derived(new Map(catStore.items.map((item) => [item.id, item])));
  const accountById = $derived(new Map(accStore.items.map((item) => [item.id, item])));

  const categoryOptions = $derived(
    catStore.items
      .filter((category) => (category.type === formType || formType === 'transfer') && !category.archived_at)
      .map((category) => ({ value: String(category.id), label: category.name })),
  );

  const visibleAccounts = $derived(accStore.items.filter((account) => !account.archived_at));

  const accountOptions = $derived(
    visibleAccounts.map((account) => ({ value: String(account.id), label: account.name })),
  );

  const counterAccountOptions = $derived(
    visibleAccounts
      .filter((account) => String(account.id) !== formAccount)
      .map((account) => ({ value: String(account.id), label: account.name })),
  );

  $effect(() => {
    if (!modalOpen) return;
    // Keep category in sync when type switches between income/expense.
    if (formType !== 'transfer' && !categoryOptions.some((option) => option.value === formCategory)) {
      formCategory = firstCategoryFor(formType);
    }
    // Keep account selections valid as the visible-accounts list changes.
    if (!accountOptions.some((option) => option.value === formAccount)) {
      formAccount = accountOptions[0]?.value ?? '';
    }
    if (formType === 'transfer' &&
        !counterAccountOptions.some((option) => option.value === formCounterAccount)) {
      formCounterAccount = counterAccountOptions[0]?.value ?? '';
    }
  });

  function labelForRow(transaction: Transaction): string {
    if (transaction.type === 'transfer') {
      const from = accountById.get(transaction.account_id)?.name ?? '?';
      const to =
        transaction.counter_account_id != null
          ? accountById.get(transaction.counter_account_id)?.name ?? '?'
          : '?';
      return `${from} → ${to}`;
    }
    if (transaction.category_id != null) {
      return categoryById.get(transaction.category_id)?.name ?? '-';
    }
    return '-';
  }

  function amountClass(type: TxType): string {
    if (type === 'income') return 'income';
    if (type === 'expense') return 'expense';
    return 'transfer';
  }

  function amountSign(type: TxType): string {
    if (type === 'income') return '+';
    if (type === 'expense') return '-';
    return '';
  }
</script>

<section>
  <header class="page-header">
    <h1>取引</h1>
    <Button onclick={openCreate}>
      {#snippet children()}+ 取引を追加{/snippet}
    </Button>
  </header>

  <Card>
    {#snippet children()}
      <div class="filters">
        <Select
          label="種別"
          bind:value={filterType}
          options={[
            { value: 'all', label: 'すべて' },
            { value: 'income', label: '収入' },
            { value: 'expense', label: '支出' },
            { value: 'transfer', label: '振替' },
          ]}
        />
        <DatePicker label="開始" bind:value={filterFrom} />
        <DatePicker label="終了" bind:value={filterTo} />
        <TextField label="フリーワード" bind:value={filterSearch} placeholder="メモを検索" />
        <Button onclick={applyFilter}>{#snippet children()}適用{/snippet}</Button>
      </div>
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      {#if txStore.items.length === 0 && !txStore.loading}
        <EmptyState title="該当する取引がありません" hint="右上から取引を追加できます" />
      {:else}
        <table data-testid="tx-table">
          <thead>
            <tr>
              <th>日付</th>
              <th>カテゴリ / 振替</th>
              <th>口座</th>
              <th>金額</th>
              <th>メモ</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each txStore.items as transaction (transaction.id)}
              <tr data-testid={`tx-row-${transaction.type}`}>
                <td>{transaction.occurred_on}</td>
                <td>{labelForRow(transaction)}</td>
                <td>
                  {accountById.get(transaction.account_id)?.name ?? '-'}
                </td>
                <td class={amountClass(transaction.type)}>
                  {amountSign(transaction.type)}{yen.format(transaction.amount)}
                </td>
                <td>{transaction.description}</td>
                <td class="actions">
                  <Button variant="ghost" onclick={() => openEdit(transaction)}>
                    {#snippet children()}編集{/snippet}
                  </Button>
                  <Button variant="ghost" onclick={() => remove(transaction)}>
                    {#snippet children()}削除{/snippet}
                  </Button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        <footer class="pager">
          <span>合計 {txStore.total} 件 / ページ {txStore.page + 1}</span>
          <span class="spacer"></span>
          <Button
            variant="ghost"
            disabled={txStore.page === 0}
            onclick={() => txStore.setPage(txStore.page - 1)}
          >
            {#snippet children()}前へ{/snippet}
          </Button>
          <Button
            variant="ghost"
            disabled={(txStore.page + 1) * txStore.pageSize >= txStore.total}
            onclick={() => txStore.setPage(txStore.page + 1)}
          >
            {#snippet children()}次へ{/snippet}
          </Button>
        </footer>
      {/if}
    {/snippet}
  </Card>
</section>

<Modal
  open={modalOpen}
  title={editing ? '取引を編集' : '取引を追加'}
  onclose={() => (modalOpen = false)}
>
  {#snippet children()}
    <Select
      label="種別"
      required
      bind:value={formType}
      options={[
        { value: 'expense', label: '支出' },
        { value: 'income', label: '収入' },
        { value: 'transfer', label: '振替' },
      ]}
      testid="tx-type"
    />
    <DatePicker label="日付" required bind:value={formDate} testid="tx-date" />
    <TextField label="金額 (円)" required type="number" bind:value={formAmount} testid="tx-amount" />
    {#if formType === 'transfer'}
      <Select
        label="振替元口座"
        required
        bind:value={formAccount}
        options={accountOptions}
        testid="tx-account"
      />
      <Select
        label="振替先口座"
        required
        bind:value={formCounterAccount}
        options={counterAccountOptions}
        testid="tx-counter-account"
      />
    {:else}
      <Select label="口座" required bind:value={formAccount} options={accountOptions} testid="tx-account" />
      <Select
        label="カテゴリ"
        required
        bind:value={formCategory}
        options={categoryOptions}
        testid="tx-category"
      />
    {/if}
    <TextField label="メモ" bind:value={formDescription} testid="tx-description" />
    {#if formError}<small class="error">{formError}</small>{/if}
  {/snippet}
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (modalOpen = false)}>
      {#snippet children()}キャンセル{/snippet}
    </Button>
    <Button onclick={() => void submit()}>
      {#snippet children()}{editing ? '更新' : '追加'}{/snippet}
    </Button>
  {/snippet}
</Modal>

<style>
  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--space-5);
  }

  h1 {
    color: white;
    margin: 0;
  }

  .filters {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: var(--space-4);
    align-items: end;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th,
  td {
    padding: var(--space-3);
    border-bottom: 1px solid var(--border);
    text-align: left;
  }

  td.income {
    color: var(--success);
    font-weight: 700;
  }

  td.expense {
    color: var(--danger);
    font-weight: 700;
  }

  td.transfer {
    color: var(--muted);
    font-weight: 700;
  }

  .actions {
    display: flex;
    gap: var(--space-2);
    justify-content: flex-end;
  }

  .pager {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-4);
  }

  .pager .spacer {
    flex: 1;
  }

  .error {
    color: var(--danger);
  }
</style>
