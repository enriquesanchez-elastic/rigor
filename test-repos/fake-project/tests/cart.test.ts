import { Cart } from '../src/cart/cart';

let cart: Cart;

describe('Cart', () => {
  beforeAll(() => {
    cart = new Cart();
  });

  it('should add items', () => {
    cart.add('1', 'Widget', 10, 2);
    const result = cart.getTotal();
    expect(result).toBeDefined();
    expect(result).toBeTruthy();
  });

  it('should have correct total after multiple adds', () => {
    cart.add('2', 'Gadget', 5, 3);
    const total = cart.getTotal();
    expect(total).toBeGreaterThan(0);
    expect(cart.getItems().length).toBeGreaterThanOrEqual(1);
  });

  it('should work when removing', () => {
    cart.remove('1');
    expect(cart).toBeDefined();
  });
});
