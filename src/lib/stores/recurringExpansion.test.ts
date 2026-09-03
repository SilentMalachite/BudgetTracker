import { beforeEach, describe, expect, it, vi } from 'vitest';

const expandDueRecurringMock = vi.fn();
vi.mock('../api/recurring', () => ({
  expandDueRecurring: (...args: unknown[]) => expandDueRecurringMock(...args),
}));

/** モジュール単位のシングルトンなので、テストごとに読み直して素の状態から始める。 */
async function freshStore() {
  vi.resetModules();
  const mod = await import('./recurringExpansion.svelte');
  return mod.recurringExpansion;
}

const empty = { generated: 0, rules: [], skipped: [] };

describe('recurring expansion store', () => {
  beforeEach(() => {
    expandDueRecurringMock.mockReset();
  });

  it('starts with nothing to show', async () => {
    const store = await freshStore();

    expect(store.result).toBeNull();
    expect(store.error).toBeNull();
    expect(store.running).toBe(false);
  });

  it('keeps a failure as a visible message instead of throwing', async () => {
    expandDueRecurringMock.mockRejectedValue(new Error('database is locked'));
    const store = await freshStore();

    await expect(store.run()).resolves.toBe(false);

    expect(store.error).toBe('database is locked');
    expect(store.result).toBeNull();
    expect(store.running).toBe(false);
  });

  it('clears the failure once a retry succeeds', async () => {
    expandDueRecurringMock.mockRejectedValueOnce(new Error('database is locked'));
    const store = await freshStore();
    await store.run();

    expandDueRecurringMock.mockResolvedValue({ ...empty, generated: 2 });
    await expect(store.run()).resolves.toBe(true);

    expect(store.error).toBeNull();
    expect(store.result).toEqual({ ...empty, generated: 2 });
  });

  it('replaces the previous result so a repaired rule stops being reported', async () => {
    expandDueRecurringMock.mockResolvedValueOnce({
      ...empty,
      skipped: [{ rule_id: 1, rule_name: '家賃', reason: 'archived_account' }],
    });
    const store = await freshStore();
    await store.run();
    expect(store.result?.skipped).toHaveLength(1);

    expandDueRecurringMock.mockResolvedValueOnce(empty);
    await store.run();

    expect(store.result?.skipped).toEqual([]);
  });
});
