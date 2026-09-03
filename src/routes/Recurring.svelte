<script lang="ts">
  import { onDestroy } from 'svelte';

  import Button from '../lib/components/Button.svelte';
  import Card from '../lib/components/Card.svelte';
  import EmptyState from '../lib/components/EmptyState.svelte';
  import ErrorBanner from '../lib/components/ErrorBanner.svelte';
  import Modal from '../lib/components/Modal.svelte';
  import Select from '../lib/components/Select.svelte';
  import TextField from '../lib/components/TextField.svelte';
  import {
    createRecurringRule,
    expandDueRecurring,
    previewRecurringOccurrences,
    updateRecurringRule,
    type ExpansionResult,
    type Frequency,
    type OccurrencePreview,
    type RecurringRuleInput,
    type RecurringRuleView,
    type SkipReason,
  } from '../lib/api/recurring';
  import { createAccountsStore } from '../lib/stores/accounts.svelte';
  import { createCategoriesStore } from '../lib/stores/categories.svelte';
  import { createRecurringStore } from '../lib/stores/recurring.svelte';
  import { isoToday } from '../lib/utils/yearMonth';

  /** 起動時展開の結果。どのルールがなぜ見送られたかを行に出すために受け取る。 */
  let { expansion = null }: { expansion?: ExpansionResult | null } = $props();

  const rules = createRecurringStore();
  const accounts = createAccountsStore();
  const categories = createCategoriesStore();

  const SKIP_REASON_LABELS: Record<SkipReason, string> = {
    archived_account: '口座がアーカイブ済み',
    archived_counter_account: '振替先口座がアーカイブ済み',
    archived_category: 'カテゴリがアーカイブ済み',
    category_type_mismatch: 'カテゴリの種別が合っていません',
    malformed_rule: 'ルールの内容が壊れています',
  };

  /** この画面で走らせた展開の結果。あれば起動時のものより新しい。 */
  let latestExpansion = $state<ExpansionResult | null>(null);

  /** rule_id -> 見送り理由。バックエンドが返した一覧を引きやすく並べ替えるだけ。 */
  const skipReasons = $derived.by(() => {
    const source = latestExpansion ?? expansion;
    const byRule = new Map<number, SkipReason>();
    for (const skipped of source?.skipped ?? []) {
      byRule.set(skipped.rule_id, skipped.reason);
    }
    return byRule;
  });

  const FREQUENCY_OPTIONS = [
    { value: 'monthly', label: '毎月' },
    { value: 'weekly', label: '毎週' },
    { value: 'yearly', label: '毎年' },
  ];

  const WEEKDAY_OPTIONS = ['日', '月', '火', '水', '木', '金', '土'].map((label, i) => ({
    value: String(i),
    label: `${label}曜日`,
  }));

  const TYPE_OPTIONS = [
    { value: 'expense', label: '支出' },
    { value: 'income', label: '収入' },
    { value: 'transfer', label: '振替' },
  ];

  function blankForm() {
    return {
      id: null as number | null,
      name: '',
      type: 'expense',
      amount: '',
      account_id: '',
      counter_account_id: '',
      category_id: '',
      description: '',
      frequency: 'monthly' as Frequency,
      day_of_month: '1',
      day_of_week: '1',
      starts_on: isoToday(),
      ends_on: '',
    };
  }

  let open = $state(false);
  let form = $state(blankForm());
  let preview = $state<OccurrencePreview | null>(null);
  let formError = $state<string | null>(null);
  let saving = $state(false);

  /** フォームの内容をコマンドの入力形に落とす。空文字は null に潰す。 */
  function toInput(): RecurringRuleInput {
    const isTransfer = form.type === 'transfer';
    const isWeekly = form.frequency === 'weekly';
    return {
      name: form.name,
      type: form.type as RecurringRuleInput['type'],
      amount: Number(form.amount),
      account_id: Number(form.account_id),
      counter_account_id: isTransfer ? Number(form.counter_account_id) : null,
      category_id: isTransfer ? null : Number(form.category_id),
      description: form.description,
      frequency: form.frequency,
      day_of_month: isWeekly ? null : Number(form.day_of_month),
      day_of_week: isWeekly ? Number(form.day_of_week) : null,
      starts_on: form.starts_on,
      ends_on: form.ends_on === '' ? null : form.ends_on,
    };
  }

  function openCreate() {
    form = blankForm();
    preview = null;
    formError = null;
    open = true;
  }

  function openEdit(view: RecurringRuleView) {
    const r = view.rule;
    form = {
      id: r.id,
      name: r.name,
      type: r.type,
      amount: String(r.amount),
      account_id: String(r.account_id),
      counter_account_id: r.counter_account_id === null ? '' : String(r.counter_account_id),
      category_id: r.category_id === null ? '' : String(r.category_id),
      description: r.description,
      frequency: r.frequency,
      day_of_month: r.day_of_month === null ? '1' : String(r.day_of_month),
      day_of_week: r.day_of_week === null ? '1' : String(r.day_of_week),
      starts_on: r.starts_on,
      ends_on: r.ends_on ?? '',
    };
    preview = null;
    formError = null;
    open = true;
  }

  /** 保存前に「今すぐ何件生成されるか」を Rust に数えさせる。 */
  async function refreshPreview() {
    formError = null;
    try {
      preview = await previewRecurringOccurrences(toInput(), 100);
    } catch (e) {
      preview = null;
      formError = e instanceof Error ? e.message : String(e);
    }
  }

  async function save() {
    saving = true;
    formError = null;
    try {
      if (form.id === null) await createRecurringRule(toInput());
      else await updateRecurringRule(form.id, toInput());

      // 新規なら、保存前に見せた「今すぐ N 件生成されます」を本当にする。編集なら、
      // 参照先を直したルールの見送りバッジをその場で消す (直したのに「壊れている」と
      // 出したままにしない) 上に、直った結果として生成されるべき分をここで生成する。
      // 展開は冪等なので起動時展開と二重にはならない。
      try {
        latestExpansion = await expandDueRecurring();
      } catch {
        // 展開の失敗で、すでに成功した保存を失敗扱いにしない。次回起動で再試行される。
      }
      open = false;
      await rules.load();
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }

  onDestroy(() => {
    void rules.dispose();
    void accounts.dispose();
    void categories.dispose();
  });
</script>

<header class="page-header">
  <h1>定期取引</h1>
  <div class="actions">
    <label>
      <input
        type="checkbox"
        checked={rules.includeInactive}
        data-testid="recurring-include-inactive"
        onchange={(e) => rules.setIncludeInactive(e.currentTarget.checked)}
      />
      停止中も表示
    </label>
    <Button onclick={openCreate} testid="recurring-new">ルールを追加</Button>
  </div>
</header>

{#if rules.error}
  <ErrorBanner message={rules.error} />
{/if}

<Card>
  {#if rules.items.length === 0}
    <EmptyState title="定期取引のルールがありません" hint="家賃や給与など、毎月同じ取引を登録できます" />
  {:else}
    <table>
      <thead>
        <tr>
          <th>名前</th>
          <th>金額</th>
          <th>周期</th>
          <th>次回予定</th>
          <th>状態</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each rules.items as view (view.rule.id)}
          <tr data-testid="recurring-row">
            <td>{view.rule.name}</td>
            <td class="num">{view.rule.amount.toLocaleString('ja-JP')} 円</td>
            <td>{FREQUENCY_OPTIONS.find((o) => o.value === view.rule.frequency)?.label}</td>
            <td data-testid="recurring-next">{view.next_occurrence ?? '—'}</td>
            <td>
              {view.rule.active ? '有効' : '停止中'}
              {#if skipReasons.has(view.rule.id)}
                <span class="skip-badge" data-testid="recurring-skip-badge">
                  見送り: {SKIP_REASON_LABELS[skipReasons.get(view.rule.id)!]}
                </span>
              {/if}
            </td>
            <td class="row-actions">
              <Button variant="ghost" onclick={() => openEdit(view)}>編集</Button>
              <Button
                variant="ghost"
                onclick={() => rules.toggleActive(view.rule.id, !view.rule.active)}
              >
                {view.rule.active ? '停止' : '再開'}
              </Button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</Card>

<Modal {open} title={form.id === null ? 'ルールを追加' : 'ルールを編集'} onclose={() => (open = false)}>
  <TextField label="名前" bind:value={form.name} required testid="recurring-name" />
  <Select label="種別" bind:value={form.type} options={TYPE_OPTIONS} testid="recurring-type" />
  <TextField label="金額 (円)" type="number" bind:value={form.amount} required testid="recurring-amount" />

  <Select
    label="口座"
    bind:value={form.account_id}
    options={accounts.items.map((a) => ({ value: String(a.id), label: a.name }))}
    required
    testid="recurring-account"
  />

  {#if form.type === 'transfer'}
    <Select
      label="振替先口座"
      bind:value={form.counter_account_id}
      options={accounts.items.map((a) => ({ value: String(a.id), label: a.name }))}
      required
      testid="recurring-counter-account"
    />
  {:else}
    <Select
      label="カテゴリ"
      bind:value={form.category_id}
      options={categories.items
        .filter((c) => c.type === form.type)
        .map((c) => ({ value: String(c.id), label: c.name }))}
      required
      testid="recurring-category"
    />
  {/if}

  <Select label="周期" bind:value={form.frequency} options={FREQUENCY_OPTIONS} testid="recurring-frequency" />

  {#if form.frequency === 'weekly'}
    <Select label="曜日" bind:value={form.day_of_week} options={WEEKDAY_OPTIONS} testid="recurring-day-of-week" />
  {:else}
    <TextField label="発生日 (1-31)" type="number" bind:value={form.day_of_month} required testid="recurring-day-of-month" />
    <p class="hint">31 を選ぶと、31 日が無い月はその月の末日になります。</p>
  {/if}

  {#if form.id !== null}
    <p class="hint" data-testid="recurring-edit-caveat">
      周期や発生日を変えると、今の期間にもう 1 件生成されることがあります。
    </p>
  {/if}

  <TextField label="開始日" type="date" bind:value={form.starts_on} required testid="recurring-starts-on" />
  <TextField label="終了日 (任意)" type="date" bind:value={form.ends_on} testid="recurring-ends-on" />
  <TextField label="メモ" bind:value={form.description} testid="recurring-description" />

  <div class="preview">
    <Button variant="ghost" onclick={refreshPreview} testid="recurring-preview-button">
      生成される日付を確認
    </Button>
    {#if preview}
      <p data-testid="recurring-preview">
        保存すると {preview.backfill_total} 件が今すぐ生成されます。次回以降は
        {preview.upcoming.join(' / ') || '予定なし'}
      </p>
    {/if}
  </div>

  {#if formError}
    <ErrorBanner message={formError} />
  {/if}

  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>キャンセル</Button>
    <Button onclick={save} disabled={saving} testid="recurring-save">保存</Button>
  {/snippet}
</Modal>

<style>
  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-4);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th,
  td {
    text-align: left;
    padding: var(--space-3);
    border-bottom: 1px solid rgba(0, 0, 0, 0.08);
  }

  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .row-actions {
    display: flex;
    gap: var(--space-2);
  }

  .hint {
    margin: 0 0 var(--space-3);
    font-size: 0.85rem;
    opacity: 0.75;
  }

  .preview {
    margin-top: var(--space-4);
  }

  .skip-badge {
    display: inline-block;
    margin-left: var(--space-2);
    padding: 0 var(--space-2);
    border-radius: var(--radius-md);
    background: rgba(255, 170, 0, 0.22);
    font-size: 0.8rem;
    white-space: nowrap;
  }
</style>
