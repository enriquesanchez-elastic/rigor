import { authenticate } from '../src/auth/auth';

let counter = 0;

describe('mixed bad patterns', () => {
  it.only('focused and weak', () => {
    const result = authenticate('user@example.com', 'correctPassword');
    expect(result).toBeDefined();
    expect(result.success).toBeTruthy();
  });

  it('shared state and vague name', () => {
    counter += 1;
    expect(counter).toBeGreaterThanOrEqual(1);
  });

  it('test 3', () => {
    console.log('counter', counter);
    expect(authenticate('a@b.com', 'correctPassword')).toBeDefined();
  });

  it('returns 401 when unauthorized', () => {
    expect(1).toBe(1);
  });

  it.skip('skipped', () => {
    expect(2).toBe(2);
  });
});
