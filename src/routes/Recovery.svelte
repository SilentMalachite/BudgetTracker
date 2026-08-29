<script lang="ts">
  import Button from '../lib/components/Button.svelte';
  import Card from '../lib/components/Card.svelte';
  import { recoverImportJson, recoverStartEmpty, type RecoveryReason } from '../lib/api/boot';

  let {
    reason,
    dbPath,
  }: {
    reason: RecoveryReason | null;
    dbPath: string;
  } = $props();

  let busy = $state(false);
  let message = $state<string | null>(null);

  async function onFileChosen(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    busy = true;
    message = null;
    try {
      await recoverImportJson(await file.text());
      window.location.reload();
    } catch (e) {
      message = `復元に失敗しました: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = false;
      input.value = '';
    }
  }

  async function startEmpty() {
    if (!confirm('空の家計簿で始めます。現在のファイルは残します。よろしいですか?')) {
      return;
    }
    busy = true;
    message = null;
    try {
      await recoverStartEmpty();
      window.location.reload();
    } catch (e) {
      message = `開始に失敗しました: ${e instanceof Error ? e.message : String(e)}`;
      busy = false;
    }
  }
</script>

<section class="recovery" data-testid="page-recovery">
  <Card>
    {#snippet children()}
      <h1>データベースを開けません</h1>
      <p>
        保存データと鍵が一致しないか、鍵が見つかりません。壊れたファイルは残します。JSON バックアップから復元するか、空の家計簿でやり直してください。
      </p>
      <dl>
        <dt>DB ファイル</dt>
        <dd data-testid="recovery-db-path">{dbPath}</dd>
        {#if reason}
          <dt>理由</dt>
          <dd data-testid="recovery-reason">{reason}</dd>
        {/if}
      </dl>
      <label class="file">
        JSON バックアップから復元
        <input
          type="file"
          accept="application/json"
          onchange={onFileChosen}
          disabled={busy}
          data-testid="recovery-import-file"
        />
      </label>
      <Button onclick={startEmpty} disabled={busy} testid="recovery-start-empty">
        {#snippet children()}空の家計簿で始める{/snippet}
      </Button>
      {#if message}
        <p class="message">{message}</p>
      {/if}
    {/snippet}
  </Card>
</section>

<style>
  .recovery {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: var(--space-6);
  }

  h1 {
    margin: 0 0 var(--space-4);
  }

  p {
    line-height: 1.6;
  }

  dl {
    display: grid;
    grid-template-columns: 8rem minmax(0, 1fr);
    gap: var(--space-2) var(--space-4);
  }

  dt {
    color: var(--muted);
    font-weight: 700;
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .file {
    display: grid;
    gap: var(--space-2);
    margin: var(--space-4) 0;
  }

  .message {
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.04);
    padding: var(--space-3);
  }
</style>
