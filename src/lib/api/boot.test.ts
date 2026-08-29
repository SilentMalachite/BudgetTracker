import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { bootStatus, recoverImportJson, recoverStartEmpty } from './boot';

describe('boot api', () => {
  beforeEach(() => invokeMock.mockReset());

  it('invokes boot_status with no args', async () => {
    invokeMock.mockResolvedValueOnce({
      state: 'ready',
      recovery_reason: null,
      db_path: '/tmp/data.db',
    });
    await bootStatus();
    expect(invokeMock).toHaveBeenCalledWith('boot_status');
  });

  it('invokes recover_import_json with payload', async () => {
    invokeMock.mockResolvedValueOnce({
      categories: 0,
      accounts: 0,
      transactions: 0,
      budgets: 0,
      warnings: [],
    });
    await recoverImportJson('{"schema_version":1}');
    expect(invokeMock).toHaveBeenCalledWith('recover_import_json', {
      payload: '{"schema_version":1}',
    });
  });

  it('invokes recover_start_empty with no args', async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await recoverStartEmpty();
    expect(invokeMock).toHaveBeenCalledWith('recover_start_empty');
  });
});
