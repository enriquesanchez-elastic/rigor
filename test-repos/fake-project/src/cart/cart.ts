export interface CartItem {
  id: string;
  name: string;
  price: number;
  quantity: number;
}

export class Cart {
  private items: CartItem[] = [];

  add(itemId: string, name: string, price: number, quantity: number): void {
    if (quantity < 1 || quantity > 100) {
      throw new Error('Quantity must be between 1 and 100');
    }
    const existing = this.items.find((i) => i.id === itemId);
    if (existing) {
      const newQty = existing.quantity + quantity;
      if (newQty > 100) throw new Error('Quantity cannot exceed 100');
      existing.quantity = newQty;
    } else {
      this.items.push({ id: itemId, name, price, quantity });
    }
  }

  remove(itemId: string): void {
    const idx = this.items.findIndex((i) => i.id === itemId);
    if (idx === -1) throw new Error('Item not in cart');
    this.items.splice(idx, 1);
  }

  getTotal(): number {
    return this.items.reduce((sum, i) => sum + i.price * i.quantity, 0);
  }

  getItems(): readonly CartItem[] {
    return [...this.items];
  }

  clear(): void {
    this.items = [];
  }
}
