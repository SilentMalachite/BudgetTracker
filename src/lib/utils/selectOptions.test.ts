import { describe, expect, it } from 'vitest';
import { optionsWithCurrent } from './selectOptions';

describe('optionsWithCurrent', () => {
  it('keeps the current archived id and does not replace it with the first active', () => {
    const items = [
      { id: 1, name: '現金', archived_at: null },
      { id: 2, name: '旧口座', archived_at: '2026-01-01T00:00:00Z' },
    ];
    const options = optionsWithCurrent(items, '2', false);
    expect(options.map((o) => o.value)).toEqual(['2', '1']);
    expect(options[0].label).toContain('アーカイブ済');
  });

  it('create path (empty current) lists only active', () => {
    const items = [
      { id: 1, name: '現金', archived_at: null },
      { id: 2, name: '旧口座', archived_at: '2026-01-01T00:00:00Z' },
    ];
    expect(optionsWithCurrent(items, '', false).map((o) => o.value)).toEqual(['1']);
  });
});
