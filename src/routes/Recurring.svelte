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
    previewRecurringOccurrences,
    updateRecurringRule,
    type Frequency,
    type OccurrencePreview,
    type RecurringRuleInput,
    type RecurringRuleView,
    type SkipReason,
  } from '../lib/api/recurring';
  import { createAccountsStore } from '../lib/stores/accounts.svelte';
  import { createCategoriesStore } from '../lib/stores/categories.svelte';
  import { createRecurringStore } from '../lib/stores/recurring.svelte';
  import { recurringExpansion } from '../lib/stores/recurringExpansion.svelte';
  import { isoToday } from '../lib/utils/yearMonth';

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

  /**
   * rule_id -> 見送り理由。バックエンドが返した一覧を引きやすく並べ替えるだけ。
   * 起動時展開もこの画面から走らせた展開も同じストアに入るので、行のバッジと
   * アプリシェルのバナーが食い違うことはない。
   */
  const skipReasons = $derived.by(() => {
    const byRule = new Map<number, SkipReason>();
    for (const skipped of recurringExpansion.result?.skipped ?? []) {
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

  /** プレビューの backfill を何件まで並べるか。件数 (`backfill_total`) は切られない。 */
  const PREVIEW_LIMIT = 100;

  function blankForm() {
    return {
      id: null as number | null,
      // 編集中のルールの watermark。展開の窓の左端 (排他) で、プレビューにそのまま渡す。
      last_generated_on: null as string | null,
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
  /** `preview` を数えたときのフォームの鍵。今の鍵と違えば、その件数はもう古い。 */
  let previewFor = $state<string | null>(null);
  /** backfill の確認待ちになっている鍵。保存はここで止まり、何も書かない。 */
  let awaitingFor = $state<string | null>(null);
  /** ユーザーが backfill を明示的に承諾した鍵。 */
  let confirmedFor = $state<string | null>(null);
  let formError = $state<string | null>(null);
  let saving = $state(false);

  /**
   * 発生日を決める項目だけを並べた鍵。1 文字でも変われば、前に数えた件数は
   * 「今のフォームの件数」ではなくなる。`type` を含めるのは、種別が変わると
   * 入力の妥当性ごと変わるため (振替は振替先、収支はカテゴリを要求する)。
   */
  const scheduleKey = $derived(
    JSON.stringify([
      form.type,
      form.frequency,
      form.day_of_month,
      form.day_of_week,
      form.starts_on,
      form.ends_on,
    ]),
  );

  /** 今のフォームに対して数えた件数だけを見せる。古い件数は表示ごと消す。 */
  const shownPreview = $derived(previewFor === scheduleKey ? preview : null);

  /** 確認待ちの backfill。フォームが動けば鍵が変わり、確認もやり直しになる。 */
  const pendingBackfill = $derived(
    awaitingFor === scheduleKey && shownPreview !== null && shownPreview.backfill_total > 0
      ? shownPreview
      : null,
  );

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

  function resetPreviewState() {
    preview = null;
    previewFor = null;
    awaitingFor = null;
    confirmedFor = null;
  }

  function openCreate() {
    form = blankForm();
    resetPreviewState();
    formError = null;
    open = true;
  }

  function openEdit(view: RecurringRuleView) {
    const r = view.rule;
    form = {
      id: r.id,
      last_generated_on: r.last_generated_on,
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
    resetPreviewState();
    formError = null;
    open = true;
  }

  /**
   * 「今すぐ何件生成されるか」を Rust に数えさせる。窓の左端は編集中のルールの
   * watermark なので、返る件数は展開が実際に作る件数と同じ。
   *
   * 数える対象は呼び出し側が切り取った `input`。フォームをここで読み直さないのは、
   * 数えたものと保存するものを 1 つの値に固定するため。
   */
  async function countOccurrences(input: RecurringRuleInput, key: string) {
    const counted = await previewRecurringOccurrences(input, PREVIEW_LIMIT, form.last_generated_on);
    preview = counted;
    previewFor = key;
    return counted;
  }

  /** 「生成される日付を確認」ボタン。数えるだけで何も書かない。 */
  async function refreshPreview() {
    formError = null;
    awaitingFor = null;
    try {
      await countOccurrences(toInput(), scheduleKey);
    } catch (e) {
      preview = null;
      previewFor = null;
      formError = e instanceof Error ? e.message : String(e);
    }
  }

  async function save() {
    saving = true;
    formError = null;
    try {
      // フォームを切り取るのはここ 1 回だけ。数えるのも書くのもこの同じ値なので、
      // 「確認した件数」と「保存した内容」が別々の入力を指すことがない。
      // 画面に出ている件数ではなく、保存する入力そのものから数え直す。開始日を
      // 打ち間違えたまま何百件も生やす事故を止められるのは、この数え直しだけ。
      const key = scheduleKey;
      const input = toInput();
      const counted = await countOccurrences(input, key);

      // IPC の往復は 1 回ぶんとはいえ待ち時間で、その間も入力は触れる。フォームが
      // 動いていたら、数えた件数も承諾も今の画面のものではない。黙って書くと
      // 「承諾していない内容」が保存され、黙って止めると押しても何も起きない画面に
      // なる。どちらも避けて、数え直しからやり直させる。
      if (scheduleKey !== key) {
        awaitingFor = null;
        formError = '入力が変わったため保存を中断しました。もう一度保存してください。';
        return;
      }

      // backfill が出るときだけ、本当の件数を見せて明示的な承諾を取る。0 件なら
      // 手順は増やさない。
      if (counted.backfill_total > 0 && confirmedFor !== key) {
        awaitingFor = key;
        return;
      }
      awaitingFor = null;

      if (form.id === null) await createRecurringRule(input);
      else await updateRecurringRule(form.id, input);

      // 新規なら、保存前に見せた「今すぐ N 件生成されます」を本当にする。編集なら、
      // 参照先を直したルールの見送りバッジをその場で消す (直したのに「壊れている」と
      // 出したままにしない) 上に、直った結果として生成されるべき分をここで生成する。
      // 展開は冪等なので起動時展開と二重にはならない。run() は失敗しても throw せず、
      // すでに成功した保存を失敗扱いにしない (失敗は下の再試行バナーに出る)。
      await recurringExpansion.run();
      open = false;
      await rules.load();
    } catch (e) {
      formError = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }

  /**
   * 「N 件を生成して保存」。承諾した鍵を控えてから保存をやり直すので、確認のあとに
   * フォームが動いていれば鍵が変わり、もう一度確認を取ることになる。
   */
  async function confirmBackfillAndSave() {
    confirmedFor = scheduleKey;
    awaitingFor = null;
    await save();
  }

  /** 失敗した展開をやり直す。起動時展開が失敗したままだと取引が 1 件も生成されない。 */
  async function retryExpansion() {
    if (await recurringExpansion.run()) await rules.load();
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

{#if recurringExpansion.error}
  <div class="expansion-error" role="alert" data-testid="recurring-expansion-error">
    <span>定期取引の展開に失敗しました: {recurringExpansion.error}</span>
    <Button
      variant="ghost"
      onclick={retryExpansion}
      disabled={recurringExpansion.running}
      testid="recurring-expansion-retry"
    >
      再試行
    </Button>
  </div>
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

  <TextField label="開始日" type="date" bind:value={form.starts_on} required testid="recurring-starts-on" />
  <TextField label="終了日 (任意)" type="date" bind:value={form.ends_on} testid="recurring-ends-on" />
  <TextField label="メモ" bind:value={form.description} testid="recurring-description" />

  <div class="preview">
    <Button variant="ghost" onclick={refreshPreview} testid="recurring-preview-button">
      生成される日付を確認
    </Button>
    {#if shownPreview}
      <p data-testid="recurring-preview">
        保存すると {shownPreview.backfill_total} 件が今すぐ生成されます。次回以降は
        {shownPreview.upcoming.join(' / ') || '予定なし'}
      </p>
    {/if}
  </div>

  {#if pendingBackfill}
    <div class="backfill-confirm" role="alert" data-testid="recurring-backfill-confirm">
      <p>
        過去にさかのぼって {pendingBackfill.backfill_total} 件の取引を今すぐ生成します。
        <!-- 日付は `backfill` の先頭と `backfill_last` から取る。`backfill` は
             PREVIEW_LIMIT で切られるので、その末尾は「limit 件目」でしかなく、
             件数が多いほど生成範囲を短く見せてしまう。 -->
        {#if pendingBackfill.backfill.length > 0 && pendingBackfill.backfill_last}
          最初は {pendingBackfill.backfill[0]}、最後は {pendingBackfill.backfill_last} です。
        {/if}
        生成される期間が意図したものか確認してください。
      </p>
      <Button
        onclick={confirmBackfillAndSave}
        disabled={saving}
        testid="recurring-backfill-confirm-button"
      >
        {pendingBackfill.backfill_total} 件を生成して保存
      </Button>
    </div>
  {/if}

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

  .backfill-confirm {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    margin-top: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    background: rgba(255, 170, 0, 0.22);
  }

  .backfill-confirm p {
    margin: 0;
  }

  .expansion-error {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    margin-bottom: var(--space-4);
    border-radius: var(--radius-md);
    background: rgba(255, 71, 87, 0.22);
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
