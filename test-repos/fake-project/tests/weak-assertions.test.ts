import { authenticate, validateAge } from '../src/auth/auth';
import { isValidEmail, parsePrice } from '../src/utils/validators';

describe('weak assertions only', () => {
  it('authenticate returns something', () => {
    const result = authenticate('user@example.com', 'correctPassword');
    expect(result).toBeDefined();
  });

  it('authenticate success is truthy', () => {
    const result = authenticate('user@example.com', 'correctPassword');
    expect(result.success).toBeTruthy();
  });

  it('invalid email is falsy', () => {
    expect(isValidEmail('bad')).toBeFalsy();
  });

  it('valid email is truthy', () => {
    expect(isValidEmail('a@b.co')).toBeTruthy();
  });

  it('parsePrice returns something', () => {
    const n = parsePrice('99');
    expect(n).toBeDefined();
    expect(n).not.toBeNull();
  });

  it('age check', () => {
    expect(validateAge(20)).toBeTruthy();
    expect(validateAge(10)).toBeFalsy();
  });
});
