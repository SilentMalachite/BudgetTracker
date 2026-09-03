<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '../lib/components/Button.svelte';
  import Card from '../lib/components/Card.svelte';
  import {
    exportBackupToFile,
    importJson,
    listPreImportSnapshots,
    restorePreImportSnapshot,
    type ImportMode,
    type ImportResult,
    type PreImportSnapshot
  } from '../lib/api/backup';
  import { getDbPath, getLastBackupAt } from '../lib/api/settings';

  let dbPath = $state('');
  let lastBackup = $state<string | null>(null);
  let busy = $state(false);
  let message = $state<string | null>(null);
  let mode = $state<ImportMode>('append');
  let warnings = $state<string[]>([]);
  let importStats = $state<ImportResult | null>(null);
  let snapshots = $state<PreImportSnapshot[]>([]);

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
    void loadSnapshots();
  });

  /**
   * Safety copies taken before overwrite imports. When the list cannot be
   * read (no Tauri runtime, or an I/O error) the section simply stays hidden.
   */
  async function loadSnapshots() {
    try {
      const list = await listPreImportSnapshots();
      snapshots = Array.isArray(list) ? list : [];
    } catch {
      snapshots = [];
    }
  }

  function formatCreatedAt(iso: string): string {
    const date = new Date(iso);
    return Number.isNaN(date.getTime()) ? iso : date.toLocaleString('ja-JP');
  }

  function formatSize(bytes: number): string {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${bytes} B`;
  }

  async function doRestore(snapshot: PreImportSnapshot) {
    const takenAt = formatCreatedAt(snapshot.created_at);
    if (
      !confirm(
        `${takenAt} 時点 (取り込み前) のデータに戻します。現在のデータは削除されず data.db.replaced-… として残ります。続行しますか?`
      )
    ) {
      return;
    }

    busy = true;
    message = null;
    importStats = null;
    warnings = [];
    try {
      await restorePreImportSnapshot(snapshot.file_name);
      message = `${takenAt} 時点のデータに戻しました`;
      lastBackup = await getLastBackupAt();
      await loadSnapshots();
    } catch (e) {
      message = `復元失敗: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = false;
    }
  }

  async function doExport() {
    busy = true;
    message = null;
    warnings = [];
    importStats = null;
    try {
      // The save dialog and the write happen in Rust; last_backup_at is only
      // recorded once the file exists, so a cancelled dialog changes nothing.
      const result = await exportBackupToFile();
      if (result) {
        lastBackup = result.last_backup_at;
        message = `バックアップを保存しました: ${result.path}`;
      }
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
    if (
      mode === 'overwrite' &&
      !confirm(
        '現在のデータをすべて置き換えます。取り込み前のデータは安全のためコピーを保存し、この画面の「取り込み前のデータに戻す」から戻せます。続行しますか?'
      )
    ) {
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
      message = `読み込み完了: カテゴリ ${result.categories} / 口座 ${result.accounts} / 定期取引 ${result.recurring_rules} / 取引 ${result.transactions} / 予算 ${result.budgets}`;
      lastBackup = await getLastBackupAt();
      if (mode === 'overwrite') {
        await loadSnapshots();
      }
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
      <label class="file">
        JSON ファイルを選択
        <input
          type="file"
          accept="application/json"
          onchange={onFileChosen}
          disabled={busy}
          data-testid="settings-import-file"
        />
      </label>

      {#if message}
        <p class="message">{message}</p>
      {/if}
      {#if importStats}
        <p class="stats">
          カテゴリ {importStats.categories} / 口座 {importStats.accounts} / 定期取引
          {importStats.recurring_rules} / 取引 {importStats.transactions} / 予算
          {importStats.budgets}
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

  {#if snapshots.length > 0}
    <Card>
      {#snippet children()}
        <h2>取り込み前のデータに戻す</h2>
        <p>
          上書きインポートの直前に保存した安全コピーです (最新 3
          件)。戻しても現在のデータは削除されず、data.db.replaced-… として残ります。
        </p>
        <ul class="snapshots" data-testid="settings-snapshots">
          {#each snapshots as snapshot (snapshot.file_name)}
            <li>
              <div class="snapshot-meta">
                <time datetime={snapshot.created_at}>{formatCreatedAt(snapshot.created_at)}</time>
                <span class="snapshot-size">{formatSize(snapshot.size_bytes)}</span>
              </div>
              <Button variant="ghost" disabled={busy} onclick={() => doRestore(snapshot)}>
                {#snippet children()}この時点に戻す{/snippet}
              </Button>
            </li>
          {/each}
        </ul>
      {/snippet}
    </Card>
  {/if}
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

  .file {
    display: grid;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }

  .message,
  .stats {
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.04);
    padding: var(--space-3);
  }

  .snapshots {
    display: grid;
    gap: var(--space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .snapshots li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .snapshot-meta {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
  }

  .snapshot-size {
    color: var(--muted);
    font-size: 0.9em;
  }

  @media (max-width: 720px) {
    dl {
      grid-template-columns: 1fr;
    }
  }
</style>
