import { isValidEmail } from '../src/utils/validators';

describe('duplicate test names', () => {
  it('validates email', () => {
    expect(isValidEmail('a@b.com')).toBe(true);
  });

  it('validates email', () => {
    expect(isValidEmail('x@y.org')).toBe(true);
  });

  it('validates email', () => {
    expect(isValidEmail('invalid')).toBe(false);
  });
});
