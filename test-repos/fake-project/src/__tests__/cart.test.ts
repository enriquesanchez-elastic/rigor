import { Cart, CartItem } from '../cart/cart';

describe('Cart', () => {
  let cart: Cart;

  beforeEach(() => {
    cart = new Cart();
  });

  describe('add', () => {
    it('adds a new item to a fresh cart with all correct properties', () => {
      cart.add('item-1', 'Widget', 9.99, 2);

      const items = cart.getItems();
      expect(items).toHaveLength(1);
      const expected: CartItem = { id: 'item-1', name: 'Widget', price: 9.99, quantity: 2 };
      expect(items[0]).toStrictEqual(expected);
    });

    it('adds multiple distinct items and computes the correct total', () => {
      cart.add('a', 'Alpha', 5, 2);
      cart.add('b', 'Beta', 10, 3);

      expect(cart.getItems()).toHaveLength(2);
      expect(cart.getTotal()).toBe(40);
    });

    it('increments quantity when adding an item with an existing id', () => {
      cart.add('item-1', 'Widget', 10, 2);
      cart.add('item-1', 'Widget', 10, 3);

      const items = cart.getItems();
      expect(items).toHaveLength(1);
      expect(items[0].quantity).toBe(5);
    });

    it('stores an item with a blank name string', () => {
      cart.add('x', '', 5, 1);

      expect(cart.getItems()[0].name).toBe('');
    });

    it('handles a null name by storing it directly', () => {
      const nullName: any = null;
      cart.add('y', nullName, 5, 1);

      expect(cart.getItems()[0].name).toBe(null);
    });

    it('throws when quantity is less than the minimum of 1', () => {
      expect(() => cart.add('x', 'X', 1, 0)).toThrow(Error);
      expect(() => cart.add('x', 'X', 1, 0)).toThrow('Quantity must be between 1 and 100');
    });

    it('accepts the minimum valid quantity of 1', () => {
      cart.add('x', 'X', 5, 1);

      expect(cart.getItems()[0].quantity).toBe(1);
    });

    it('accepts the maximum valid quantity of 100', () => {
      cart.add('x', 'X', 5, 100);

      expect(cart.getItems()[0].quantity).toBe(100);
    });

    it('throws when quantity exceeds the maximum of 100', () => {
      expect(() => cart.add('x', 'X', 1, 101)).toThrow(Error);
      expect(() => cart.add('x', 'X', 1, 101)).toThrow('Quantity must be between 1 and 100');
    });

    it('throws for negative quantity values', () => {
      expect(() => cart.add('x', 'X', 1, -5)).toThrow(Error);
      expect(() => cart.add('x', 'X', 1, -5)).toThrow('Quantity must be between 1 and 100');
    });

    it('throws when combined quantity of an existing item would exceed the limit', () => {
      cart.add('x', 'X', 1, 60);

      expect(() => cart.add('x', 'X', 1, 50)).toThrow(Error);
      expect(() => cart.add('x', 'X', 1, 50)).toThrow('Quantity cannot exceed 100');
    });

    it('allows combined quantity to reach exactly the limit', () => {
      cart.add('x', 'X', 1, 50);
      cart.add('x', 'X', 1, 50);

      expect(cart.getItems()[0].quantity).toBe(100);
    });
  });

  describe('remove', () => {
    it('removes an existing item so the cart has no remaining items', () => {
      cart.add('item-1', 'Widget', 5, 1);
      cart.remove('item-1');

      expect(cart.getItems()).toHaveLength(0);
      expect(cart.getTotal()).toBe(0);
    });

    it('throws when removing an item that is not in the cart', () => {
      expect(() => cart.remove('nonexistent')).toThrow(Error);
      expect(() => cart.remove('nonexistent')).toThrow('Item not in cart');
    });

    it('removes only the targeted item and leaves others intact', () => {
      cart.add('a', 'Alpha', 5, 1);
      cart.add('b', 'Beta', 10, 1);
      cart.remove('a');

      const items = cart.getItems();
      expect(items).toHaveLength(1);
      expect(items[0].id).toBe('b');
    });
  });

  describe('getTotal', () => {
    it('has an initial total before any items are added', () => {
      expect(cart.getTotal()).toBe(0);
    });

    it('computes price times quantity for a single item', () => {
      cart.add('x', 'X', 12.5, 4);

      expect(cart.getTotal()).toBe(50);
    });

    it('sums across multiple items with different prices and quantities', () => {
      cart.add('a', 'A', 10, 2);
      cart.add('b', 'B', 5.5, 3);

      expect(cart.getTotal()).toBe(36.5);
    });

    it('handles very small fractional prices', () => {
      cart.add('penny', 'Penny Item', 0.01, 1);

      expect(cart.getTotal()).toBe(0.01);
    });
  });

  describe('getItems', () => {
    it('returns a list with no entries for a new cart', () => {
      expect(cart.getItems()).toHaveLength(0);
      expect(cart.getItems()).toEqual([]);
    });

    it('returns a defensive copy that is not the same reference as another call', () => {
      cart.add('x', 'X', 1, 1);
      const first = cart.getItems();
      const second = cart.getItems();

      expect(first).toEqual(second);
      expect(first).not.toBe(second);
    });
  });

  describe('clear', () => {
    it('removes all items and resets the total', () => {
      cart.add('a', 'A', 5, 1);
      cart.add('b', 'B', 10, 2);
      cart.clear();

      expect(cart.getItems()).toHaveLength(0);
      expect(cart.getItems()).toEqual([]);
      expect(cart.getTotal()).toBe(0);
    });

    it('is safe to call on a cart with no items', () => {
      cart.clear();

      expect(cart.getItems()).toHaveLength(0);
    });
  });
});
