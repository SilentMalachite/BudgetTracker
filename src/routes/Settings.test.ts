import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const importJsonMock = vi.fn();

vi.mock('../lib/api/backup', () => ({
  exportBackupToFile: vi.fn(),
  importJson: (...args: unknown[]) => importJsonMock(...args),
  listPreImportSnapshots: async () => [],
  restorePreImportSnapshot: vi.fn(),
}));
vi.mock('../lib/api/settings', () => ({
  getDbPath: async () => '/tmp/data.db',
  getLastBackupAt: async () => null,
}));

import Settings from './Settings.svelte';

const result = (warnings: string[]) => ({
  categories: 1,
  accounts: 1,
  recurring_rules: 0,
  transactions: 2,
  budgets: 0,
  warnings,
});

/**
 * JSON ファイルを 1 つ選んだことにする。jsdom の `File` は `text()` を実装して
 * いないので、画面が使う `size` と `text()` だけを持つ最小の代役を渡す。
 */
async function chooseFile() {
  const input = screen.getByTestId('settings-import-file') as HTMLInputElement;
  const file = { name: 'backup.json', size: 2, text: async () => '{}' };
  Object.defineProperty(input, 'files', { value: [file], configurable: true });
  await fireEvent.change(input);
}

describe('Settings のインポート結果', () => {
  beforeEach(() => {
    importJsonMock.mockReset();
    vi.stubGlobal('confirm', () => true);
  });

  it('追記でまとめた行があるときだけ、まとめた旨を案内する', async () => {
    importJsonMock.mockResolvedValue(result(['取引 3 件は既存の行と重複したため追加しませんでした']));

    render(Settings);
    await chooseFile();

    await waitFor(() => {
      expect(screen.getByText(/既存の行にまとめています/)).toBeTruthy();
    });
  });

  it('警告が無ければ「警告を確認してください」と言わない', async () => {
    importJsonMock.mockResolvedValue(result([]));

    render(Settings);
    await chooseFile();

    await waitFor(() => {
      expect(importJsonMock).toHaveBeenCalled();
    });
    expect(screen.queryByText(/既存の行にまとめています/)).toBeNull();
  });

  it('上書きは既存を消してから入れるので、まとめた旨を出さない', async () => {
    importJsonMock.mockResolvedValue(result(['取引 3 件は既存の行と重複したため追加しませんでした']));

    render(Settings);
    await fireEvent.click(screen.getByLabelText('上書き'));
    await chooseFile();

    await waitFor(() => {
      expect(importJsonMock).toHaveBeenCalledWith(expect.any(String), 'overwrite');
    });
    expect(screen.queryByText(/既存の行にまとめています/)).toBeNull();
  });
});
