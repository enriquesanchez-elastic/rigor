import { Cart } from '../src/cart/cart';

let cart: Cart;
let totalItems = 0;

describe('shared mutable state', () => {
  it('first test adds item', () => {
    cart = new Cart();
    cart.add('1', 'A', 10, 1);
    totalItems += 1;
    expect(cart.getTotal()).toBe(10);
  });

  it('second test depends on first', () => {
    cart.add('2', 'B', 5, 1);
    totalItems += 1;
    expect(cart.getTotal()).toBe(15);
    expect(totalItems).toBe(2);
  });

  it('third test expects accumulated state', () => {
    expect(cart.getItems()).toHaveLength(2);
    expect(totalItems).toBe(2);
  });
});
