describe('Checkout', () => {
  beforeEach(() => {
    cy.visit('/cart');
  });

  it('shows cart items', () => {
    cy.get('[data-cy="cart-item"]').should('exist');
  });

  it('proceeds to checkout', () => {
    cy.get('button').contains('Checkout').click();
    cy.url().should('include', '/checkout');
    cy.get('.checkout-form').should('exist');
  });

  it('displays total', () => {
    cy.get('[data-cy="cart-total"]').should('be.visible');
    cy.get('[data-cy="cart-total"]').should('contain', '$');
  });
});
