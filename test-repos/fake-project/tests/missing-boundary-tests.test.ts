import { validateAge } from '../src/auth/auth';
import { clamp } from '../src/utils/validators';

describe('boundary in source but not tested', () => {
  it('validateAge only tests one value - no 17, 18, 19', () => {
    expect(validateAge(25)).toBe(true);
  });

  it('clamp only tests middle - no min/max boundary', () => {
    expect(clamp(5, 0, 10)).toBe(5);
  });
});
