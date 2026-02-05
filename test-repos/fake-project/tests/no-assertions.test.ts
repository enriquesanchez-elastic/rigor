import { authenticate } from '../src/auth/auth';
import { Cart } from '../src/cart/cart';

describe('tests with no assertions', () => {
  it('calls authenticate', () => {
    authenticate('user@example.com', 'correctPassword');
  });

  it('creates cart and adds item', () => {
    const cart = new Cart();
    cart.add('1', 'Foo', 10, 1);
  });

  it('does nothing', () => {
    const x = 1 + 1;
  });

  it('only sets a variable', () => {
    let result: unknown;
    result = authenticate('a@b.com', 'correctPassword');
  });
});
