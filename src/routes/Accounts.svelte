<script lang="ts">
  import { onDestroy } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import Card from '../lib/components/Card.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import Select from '../lib/components/Select.svelte';
  import TextField from '../lib/components/TextField.svelte';
  import {
    ACCOUNT_KIND_ICONS,
    ACCOUNT_KIND_LABELS,
    archiveAccount,
    createAccount,
    unarchiveAccount,
    updateAccount,
    type Account,
    type AccountKind,
  } from '../lib/api/accounts';
  import { createAccountsStore } from '../lib/stores/accounts.svelte';
  import { createBalancesStore } from '../lib/stores/balances.svelte';

  const store = createAccountsStore(true);
  const balances = createBalancesStore();
  onDestroy(() => {
    void store.dispose();
    void balances.dispose();
  });

  const yen = new Intl.NumberFormat('ja-JP', { style: 'currency', currency: 'JPY' });
  const kindOptions = (Object.keys(ACCOUNT_KIND_LABELS) as AccountKind[]).map((kind) => ({
    value: kind,
    label: `${ACCOUNT_KIND_ICONS[kind]} ${ACCOUNT_KIND_LABELS[kind]}`,
  }));

  let modalOpen = $state(false);
  let editing = $state<Account | null>(null);
  let name = $state('');
  let kind = $state<AccountKind>('cash');
  let initialBalance = $state('0');
  let note = $state('');
  let formError = $state<string | null>(null);

  function openCreate() {
    editing = null;
    name = '';
    kind = 'cash';
    initialBalance = '0';
    note = '';
    formError = null;
    modalOpen = true;
  }

  function openEdit(account: Account) {
    editing = account;
    name = account.name;
    kind = account.kind;
    initialBalance = String(account.initial_balance);
    note = account.note;
    formError = null;
    modalOpen = true;
  }

  async function submit() {
    formError = null;
    const balanceText = String(initialBalance).trim();
    if (!/^-?\d+$/.test(balanceText)) {
      formError = '開始残高は整数を入力してください';
      return;
    }
    const balance = Number.parseInt(balanceText, 10);
    try {
      if (editing) {
        await updateAccount(editing.id, {
          name,
          kind,
          initial_balance: balance,
          note,
        });
      } else {
        await createAccount({ name, kind, initial_balance: balance, note });
      }
      modalOpen = false;
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    }
  }

  async function toggleArchive(account: Account) {
    if (account.archived_at) await unarchiveAccount(account.id);
    else await archiveAccount(account.id);
  }

  const visible = $derived(store.items.filter((account) => !account.archived_at));
  const archived = $derived(store.items.filter((account) => !!account.archived_at));
  const balanceById = $derived(
    new Map(balances.items.map((row) => [row.account_id, row.balance])),
  );
</script>

<section>
  <header class="page-header">
    <h1>口座</h1>
    <Button onclick={openCreate}>
      {#snippet children()}+ 追加{/snippet}
    </Button>
  </header>

  <Card>
    {#snippet children()}
      {#if visible.length === 0 && !store.loading}
        <EmptyState title="口座がありません" hint="右上の「+ 追加」から作成してください" />
      {:else}
        <ul class="list" data-testid="accounts-list">
          {#each visible as account (account.id)}
            <li>
              <span class="icon" aria-hidden="true">{ACCOUNT_KIND_ICONS[account.kind]}</span>
              <div class="meta">
                <strong>{account.name}</strong>
                <small>{ACCOUNT_KIND_LABELS[account.kind]}</small>
              </div>
              <span class="balance" data-testid={`account-balance-${account.id}`}>
                <strong>{yen.format(balanceById.get(account.id) ?? account.initial_balance)}</strong>
                {#if (balanceById.get(account.id) ?? account.initial_balance) !== account.initial_balance}
                  <small>初期 {yen.format(account.initial_balance)}</small>
                {/if}
              </span>
              <Button variant="ghost" onclick={() => openEdit(account)}>
                {#snippet children()}編集{/snippet}
              </Button>
              <Button variant="ghost" onclick={() => toggleArchive(account)}>
                {#snippet children()}アーカイブ{/snippet}
              </Button>
            </li>
          {/each}
        </ul>
      {/if}
    {/snippet}
  </Card>

  {#if archived.length > 0}
    <h2 class="sub">アーカイブ済み</h2>
    <Card>
      {#snippet children()}
        <ul class="list">
          {#each archived as account (account.id)}
            <li>
              <span class="icon" aria-hidden="true">{ACCOUNT_KIND_ICONS[account.kind]}</span>
              <div class="meta">
                <strong>{account.name}</strong>
                <small>{ACCOUNT_KIND_LABELS[account.kind]}</small>
              </div>
              <span class="balance"><strong>{yen.format(account.initial_balance)}</strong></span>
              <Button variant="ghost" onclick={() => toggleArchive(account)}>
                {#snippet children()}復元{/snippet}
              </Button>
            </li>
          {/each}
        </ul>
      {/snippet}
    </Card>
  {/if}
</section>

<Modal
  open={modalOpen}
  title={editing ? '口座を編集' : '口座を追加'}
  onclose={() => (modalOpen = false)}
>
  {#snippet children()}
    <TextField label="名前" required bind:value={name} testid="account-name" />
    <Select
      label="種別"
      required
      bind:value={kind}
      options={kindOptions}
      testid="account-kind"
    />
    <TextField
      label="開始残高 (円)"
      required
      type="number"
      bind:value={initialBalance}
      testid="account-initial"
    />
    <TextField label="メモ" bind:value={note} />
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

  h2.sub {
    color: white;
    margin: var(--space-6) 0 var(--space-4);
  }

  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: var(--space-3);
  }

  .list li {
    display: grid;
    grid-template-columns: auto 1fr auto auto auto;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: rgba(0, 0, 0, 0.02);
  }

  .icon {
    font-size: 1.5rem;
  }

  .meta {
    display: grid;
  }

  .meta small {
    color: var(--muted);
  }

  .balance {
    font-variant-numeric: tabular-nums;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    line-height: 1.2;
  }
  .balance strong {
    font-weight: 700;
  }
  .balance small {
    color: var(--muted);
  }

  .error {
    color: var(--danger);
  }
</style>
