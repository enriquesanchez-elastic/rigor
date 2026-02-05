import { describe, it, expect, vi } from 'vitest';

function add(a: number, b: number): number {
  return a + b;
}

function maybeRandom(): number {
  return Math.random() > 0.5 ? 1 : 0;
}

describe('math', () => {
  it('returns sum of two positive numbers', () => {
    expect(add(1, 2)).toBe(3);
  });

  it('returns 0 when both operands are 0', () => {
    const result = add(0, 0);
    expect(result).toBe(0);
  });

  it('returns 5 for 2 + 3', () => {
    expect(add(2, 3)).toBe(5);
  });

  it('returns 1 when Math.random is above 0.5', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.7);
    expect(maybeRandom()).toBe(1);
    vi.restoreAllMocks();
  });

  it('returns 0 when Math.random is at or below 0.5', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.5);
    expect(maybeRandom()).toBe(0);
    vi.restoreAllMocks();
  });
});
