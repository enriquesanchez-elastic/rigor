describe('Cypress weak assertions', () => {
  beforeEach(() => {
    cy.visit('/');
  });

  it('element exists', () => {
    cy.get('.header').should('exist');
  });

  it('just get no should', () => {
    cy.get('[data-cy="main"]');
  });

  it('should exist again', () => {
    cy.get('body').should('exist');
  });

  it('visible but weak', () => {
    cy.get('.content').should('be.visible');
  });
});
