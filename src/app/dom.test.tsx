// SPDX-License-Identifier: MPL-2.0
import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';

it('runs .test.tsx files in jsdom', () => {
  render(<button>Hi</button>);
  expect(typeof document).toBe('object');
  expect(screen.getByRole('button', { name: 'Hi' })).toBeTruthy();
});
