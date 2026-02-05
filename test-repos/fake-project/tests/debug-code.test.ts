import { authenticate } from '../src/auth/auth';

describe('debug code in tests', () => {
  it('logs result', () => {
    const result = authenticate('user@example.com', 'correctPassword');
    console.log('result', result);
    expect(result.success).toBe(true);
  });

  it('has debugger', () => {
    const x = 1;
    debugger;
    expect(x).toBe(1);
  });

  it('warns', () => {
    console.warn('checking auth');
    expect(authenticate('a@b.com', 'correctPassword')).toBeDefined();
  });

  it('debug and log', () => {
    console.debug('state', { user: 'test' });
    expect(1).toBe(1);
  });
});
