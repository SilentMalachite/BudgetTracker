import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

import Modal from './Modal.svelte';

describe('Modal', () => {
  it('focuses the first input when opened and calls onclose on Escape', async () => {
    const onclose = vi.fn();
    const children = createRawSnippet(() => ({
      render: () => `<input data-testid="modal-input" />`,
    }));

    render(Modal, {
      props: {
        open: true,
        title: 'テスト',
        onclose,
        children,
      },
    });

    const input = screen.getByTestId('modal-input');
    await waitFor(() => {
      expect(document.activeElement).toBe(input);
    });

    await fireEvent.keyDown(document, { key: 'Escape' });
    expect(onclose).toHaveBeenCalledTimes(1);
  });
});
