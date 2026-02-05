import { isValidEmail, clamp } from '../src/utils/validators';

describe('vague test names', () => {
  it('test 1', () => {
    expect(isValidEmail('a@b.com')).toBe(true);
  });

  it('test 2', () => {
    expect(isValidEmail('bad')).toBe(false);
  });

  it('should work', () => {
    expect(clamp(5, 0, 10)).toBe(5);
  });

  it('works', () => {
    expect(clamp(-1, 0, 10)).toBe(0);
  });

  it('test', () => {
    expect(clamp(99, 0, 10)).toBe(10);
  });

  it('case 1', () => {
    expect(1).toBe(1);
  });

  it('case 2', () => {
    expect(2).toBe(2);
  });
});
