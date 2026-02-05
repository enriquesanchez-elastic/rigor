import { authenticate } from '../src/auth/auth';
import { isValidEmail, parsePrice, clamp } from '../src/utils/validators';

describe('limited input variety - same hardcoded values', () => {
  it('always uses same email', () => {
    expect(authenticate('user@example.com', 'correctPassword').success).toBe(true);
  });

  it('same email again', () => {
    expect(authenticate('user@example.com', 'correctPassword').user?.email).toBe('user@example.com');
  });

  it('yet again user@example.com', () => {
    const result = authenticate('user@example.com', 'correctPassword');
    expect(result.token).toHaveLength(64);
  });

  it('validates one email only', () => {
    expect(isValidEmail('user@example.com')).toBe(true);
  });

  it('single price input', () => {
    expect(parsePrice('99.99')).toBe(99.99);
  });

  it('clamp with one value', () => {
    expect(clamp(5, 0, 10)).toBe(5);
  });
});

describe('missing edge cases', () => {
  it('no test for empty string', () => {
    expect(isValidEmail('user@example.com')).toBe(true);
  });

  it('no test for zero', () => {
    expect(clamp(1, 0, 10)).toBe(1);
  });

  it('no test for null or negative', () => {
    expect(parsePrice('0')).toBe(0);
  });
});
