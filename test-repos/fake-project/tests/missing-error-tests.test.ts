import { authenticate, AuthError } from '../src/auth/auth';
import { parsePrice, ParseError } from '../src/utils/validators';
import { Cart } from '../src/cart/cart';

describe('source throws but no error tests', () => {
  it('authenticate success only - no test for invalid credentials', () => {
    const result = authenticate('user@example.com', 'correctPassword');
    expect(result.success).toBe(true);
  });

  it('parsePrice success only - no test for ParseError', () => {
    expect(parsePrice('100')).toBe(100);
  });

  it('Cart add success only - no test for quantity out of range', () => {
    const cart = new Cart();
    cart.add('1', 'Item', 10, 1);
    expect(cart.getTotal()).toBe(10);
  });
});
