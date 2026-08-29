import { describe, expect, it } from 'vitest';
import { readyBootResult } from './e2e/tauriMock';

describe('ready boot e2e mock', () => {
  it('returns the current BalanceList shape for list_balances', () => {
    expect(readyBootResult('list_balances')).toEqual({ accounts: [], total_assets: 0 });
  });
});
