import { authenticate } from '../src/auth/auth';

describe.only('focused describe', () => {
  it('runs', () => {
    expect(authenticate('a@b.com', 'correctPassword').success).toBe(true);
  });
});

describe('skipped and focused tests', () => {
  it.skip('skipped test', () => {
    expect(1).toBe(1);
  });

  it.only('only this one', () => {
    expect(2).toBe(2);
  });

  fit('fit alias', () => {
    expect(3).toBe(3);
  });

  xit('xit skipped', () => {
    expect(4).toBe(4);
  });

  describe.skip('skipped describe', () => {
    it('inside skipped', () => {
      expect(5).toBe(5);
    });
  });
});
