import { authenticate } from '../src/auth/auth';

describe('flaky patterns', () => {
  it('uses Date.now()', () => {
    const start = Date.now();
    authenticate('user@example.com', 'correctPassword');
    const elapsed = Date.now() - start;
    expect(elapsed).toBeGreaterThanOrEqual(0);
  });

  it('uses Math.random()', () => {
    const r = Math.random();
    expect(r).toBeGreaterThanOrEqual(0);
    expect(r).toBeLessThanOrEqual(1);
  });

  it('uses setTimeout', () => {
    let done = false;
    setTimeout(() => {
      done = true;
    }, 10);
    expect(done).toBe(false);
  });

  it('uses setInterval', () => {
    const id = setInterval(() => {}, 100);
    clearInterval(id);
    expect(id).toBeDefined();
  });

  it('calls fetch without mock', () => {
    const url = 'https://api.example.com/user';
    fetch(url).then((res) => {
      expect(res).toBeDefined();
    });
  });
});
