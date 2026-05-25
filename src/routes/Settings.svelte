<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import Card from '../lib/components/Card.svelte';
  import { exportJson, importJson, type ImportMode, type ImportResult } from '../lib/api/backup';
  import { getDbPath, getLastBackupAt, setLastBackupAt } from '../lib/api/settings';

  let dbPath = $state('');
  let lastBackup = $state<string | null>(null);
  let busy = $state(false);
  let message = $state<string | null>(null);
  let mode = $state<ImportMode>('append');
  let warnings = $state<string[]>([]);
  let importStats = $state<ImportResult | null>(null);

  onMount(() => {
    void (async () => {
      try {
        const [path, backupAt] = await Promise.all([getDbPath(), getLastBackupAt()]);
        dbPath = path;
        lastBackup = backupAt;
      } catch (e) {
        message = `設定情報の読み込みに失敗しました: ${e instanceof Error ? e.message : String(e)}`;
      }
    })();
  });

  async function doExport() {
    busy = true;
    message = null;
    warnings = [];
    importStats = null;
    try {
      const json = await exportJson();
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      const stamp = new Date().toISOString().replace(/[:.]/g, '-');
      link.href = url;
      link.download = `budget-backup-${stamp}.json`;
      link.click();
      URL.revokeObjectURL(url);

      const now = new Date().toISOString();
      await setLastBackupAt(now);
      lastBackup = now;
      message = 'バックアップを書き出しました';
    } catch (e) {
      message = `エクスポート失敗: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = false;
    }
  }

  async function onFileChosen(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    if (file.size > 10 * 1024 * 1024) {
      message = 'ファイルサイズが 10MB を超えています';
      input.value = '';
      return;
    }
    if (mode === 'overwrite' && !confirm('現在のデータをすべて置き換えます。続行しますか?')) {
      input.value = '';
      return;
    }

    busy = true;
    message = null;
    importStats = null;
    warnings = [];
    try {
      const result = await importJson(await file.text(), mode);
      importStats = result;
      warnings = result.warnings;
      message = `読み込み完了: カテゴリ ${result.categories} / 口座 ${result.accounts} / 取引 ${result.transactions} / 予算 ${result.budgets}`;
      lastBackup = await getLastBackupAt();
    } catch (e) {
      message = `インポート失敗: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = false;
      input.value = '';
    }
  }
</script>

<section data-testid="page-settings">
  <h1>設定</h1>

  <Card>
    {#snippet children()}
      <h2>データ</h2>
      <dl>
        <dt>DB ファイル</dt>
        <dd data-testid="settings-db-path">{dbPath || '読み込み中'}</dd>
        <dt>最終バックアップ</dt>
        <dd data-testid="settings-last-backup">{lastBackup ?? '未実施'}</dd>
      </dl>
    {/snippet}
  </Card>

  <Card>
    {#snippet children()}
      <h2>バックアップ</h2>
      <p>
        JSON で全データをエクスポート / 復元します。鍵紛失時の復旧手段なので、定期的にバックアップしてください。
      </p>

      <div class="actions">
        <Button onclick={doExport} disabled={busy}>
          {#snippet children()}JSON をエクスポート{/snippet}
        </Button>
      </div>

      <h3>インポート</h3>
      <label class="mode">
        <input type="radio" name="import-mode" value="append" bind:group={mode} />
        追記
      </label>
      <label class="mode">
        <input type="radio" name="import-mode" value="overwrite" bind:group={mode} />
        上書き
      </label>
      <input
        type="file"
        accept="application/json"
        onchange={onFileChosen}
        disabled={busy}
        data-testid="settings-import-file"
      />

      {#if message}
        <p class="message">{message}</p>
      {/if}
      {#if importStats}
        <p class="stats">
          カテゴリ {importStats.categories} / 口座 {importStats.accounts} / 取引
          {importStats.transactions} / 予算 {importStats.budgets}
        </p>
      {/if}
      {#if warnings.length > 0}
        <details>
          <summary>{warnings.length} 件の警告</summary>
          <ul>
            {#each warnings as warning}
              <li>{warning}</li>
            {/each}
          </ul>
        </details>
      {/if}
    {/snippet}
  </Card>
</section>

<style>
  section {
    display: grid;
    gap: var(--space-5);
  }

  h1 {
    color: white;
    margin: 0;
  }

  h2,
  h3 {
    margin-top: 0;
  }

  dl {
    display: grid;
    grid-template-columns: 10rem minmax(0, 1fr);
    gap: var(--space-3) var(--space-4);
  }

  dt {
    color: var(--muted);
    font-weight: 700;
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    gap: var(--space-3);
    margin: var(--space-4) 0;
  }

  .mode {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    padding: var(--space-1) 0;
  }

  input[type='file'] {
    margin-top: var(--space-3);
  }

  .message,
  .stats {
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.04);
    padding: var(--space-3);
  }

  @media (max-width: 720px) {
    dl {
      grid-template-columns: 1fr;
    }
  }
</style>
