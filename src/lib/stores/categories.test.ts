import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const listMock = vi.fn();
const onChangedMock = vi.fn();

vi.mock('../api/categories', () => ({
  listCategories: (...args: unknown[]) => listMock(...args),
}));

vi.mock('../api/events', () => ({
  onDataChanged: (cb: (d: string) => void) => onChangedMock(cb),
}));

import { createCategoriesStore } from './categories.svelte';

describe('categories store', () => {
  beforeEach(() => {
    listMock.mockReset();
    onChangedMock.mockReset();
    onChangedMock.mockImplementation(() => Promise.resolve(() => {}));
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('loads items on construction', async () => {
    listMock.mockResolvedValueOnce([
      {
        id: 1,
        name: '食費',
        type: 'expense',
        color: null,
        icon: null,
        display_order: 0,
        archived_at: null,
      },
    ]);
    const store = createCategoriesStore();
    await vi.waitFor(() => expect(store.items).toHaveLength(1));
    expect(store.items[0].name).toBe('食費');
    await store.dispose();
  });

  it('re-fetches when data:changed fires for categories', async () => {
    let trigger: ((domain: string) => void) | undefined;
    onChangedMock.mockImplementation((cb: (d: string) => void) => {
      trigger = cb;
      return Promise.resolve(() => {});
    });
    listMock.mockResolvedValue([]);

    const store = createCategoriesStore();
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(onChangedMock).toHaveBeenCalledTimes(1));
    listMock.mockResolvedValueOnce([
      {
        id: 2,
        name: '給与',
        type: 'income',
        color: null,
        icon: null,
        display_order: 0,
        archived_at: null,
      },
    ]);
    trigger?.('categories');
    await vi.waitFor(() => expect(store.items[0]?.name).toBe('給与'));
    await store.dispose();
  });

  it('ignores a stale list response', async () => {
    listMock.mockResolvedValueOnce([]);

    const store = createCategoriesStore();
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));

    let resolveOld: (value: unknown) => void = () => {};
    const oldPromise = new Promise((resolve) => {
      resolveOld = resolve;
    });
    listMock.mockImplementationOnce(() => oldPromise);
    listMock.mockResolvedValueOnce([
      {
        id: 2,
        name: '給与',
        type: 'income',
        color: null,
        icon: null,
        display_order: 0,
        archived_at: null,
      },
    ]);

    store.setFilter({ type: 'expense' });
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(2));
    store.setFilter({ type: 'income' });
    await vi.waitFor(() => expect(store.items[0]?.name).toBe('給与'));

    resolveOld([
      {
        id: 1,
        name: '食費',
        type: 'expense',
        color: null,
        icon: null,
        display_order: 0,
        archived_at: null,
      },
    ]);
    await new Promise((r) => setTimeout(r, 20));
    expect(store.items[0]?.name).toBe('給与');
    await store.dispose();
  });

  it('ignores unrelated domain changes', async () => {
    let trigger: ((domain: string) => void) | undefined;
    onChangedMock.mockImplementation((cb: (d: string) => void) => {
      trigger = cb;
      return Promise.resolve(() => {});
    });
    listMock.mockResolvedValue([]);
    const store = createCategoriesStore();
    await vi.waitFor(() => expect(listMock).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(onChangedMock).toHaveBeenCalledTimes(1));
    trigger?.('accounts');
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(listMock).toHaveBeenCalledTimes(1);
    await store.dispose();
  });
});
