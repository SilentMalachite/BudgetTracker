import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { importJson, listPreImportSnapshots, restorePreImportSnapshot } from './backup';

describe('backup api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('wraps payload and mode under args for import_json', async () => {
    invokeMock.mockResolvedValueOnce({
      categories: 0,
      accounts: 0,
      transactions: 0,
      budgets: 0,
      warnings: [],
    });

    await importJson('{"schema_version":1}', 'overwrite');

    expect(invokeMock).toHaveBeenCalledWith('import_json', {
      args: { payload: '{"schema_version":1}', mode: 'overwrite' },
    });
  });

  it('invokes list_pre_import_snapshots with no args', async () => {
    const snapshots = [
      {
        file_name: 'data.db.pre-import-20260903T120000Z',
        created_at: '2026-09-03T12:00:00Z',
        size_bytes: 4096,
      },
    ];
    invokeMock.mockResolvedValueOnce(snapshots);

    await expect(listPreImportSnapshots()).resolves.toEqual(snapshots);

    expect(invokeMock).toHaveBeenCalledWith('list_pre_import_snapshots');
  });

  it('passes fileName through to restore_pre_import_snapshot', async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await restorePreImportSnapshot('data.db.pre-import-20260903T120000Z');

    expect(invokeMock).toHaveBeenCalledWith('restore_pre_import_snapshot', {
      fileName: 'data.db.pre-import-20260903T120000Z',
    });
  });
});
