import { Cart } from '../src/cart/cart';
import { clamp } from '../src/utils/validators';

describe('mutation-resistant assertions', () => {
  it('toBeGreaterThan(0) instead of exact value', () => {
    const cart = new Cart();
    cart.add('1', 'A', 10, 2);
    expect(cart.getTotal()).toBeGreaterThan(0);
  });

  it('toBeLessThanOrEqual(100) instead of exact', () => {
    expect(clamp(50, 0, 100)).toBeLessThanOrEqual(100);
  });

  it('toHaveLength greater than 0', () => {
    const cart = new Cart();
    cart.add('1', 'X', 1, 1);
    expect(cart.getItems()).toHaveLength(1);
  });

  it('truthy instead of specific', () => {
    expect(clamp(5, 0, 10) >= 0).toBeTruthy();
  });
});
