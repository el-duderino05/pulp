// SPDX-License-Identifier: MPL-2.0
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ErrorBanner from './ErrorBanner';

afterEach(cleanup);

describe('ErrorBanner', () => {
  it('shows the message in an alert', () => {
    render(<ErrorBanner message="Nope" onDismiss={() => {}} />);
    expect(screen.getByRole('alert').textContent).toContain('Nope');
  });
  it('can be dismissed', async () => {
    const onDismiss = vi.fn();
    render(<ErrorBanner message="Nope" onDismiss={onDismiss} />);
    await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(onDismiss).toHaveBeenCalled();
  });
});
