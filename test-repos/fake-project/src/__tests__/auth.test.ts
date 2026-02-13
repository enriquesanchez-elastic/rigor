import { authenticate, validateAge, createSession, AuthError } from '../auth/auth';

describe('authenticate', () => {
  beforeEach(() => {
    jest.useFakeTimers();
    jest.setSystemTime(new Date('2024-06-15T12:00:00Z'));
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it('returns success payload with user data and token for valid credentials', () => {
    const now = Date.now();
    const result = authenticate('a@b.com', 'correctPassword');

    expect(result).toStrictEqual({
      success: true,
      user: { id: `user_${now}`, email: 'a@b.com' },
      token: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    });
  });

  it('includes the provided email in the user and always returns the same token shape', () => {
    const result = authenticate('a@b.com', 'correctPassword');

    expect(result.success).toBe(true);
    expect(result.user!.email).toBe('a@b.com');
    expect(result.token).toHaveLength(64);
  });

  it('throws AuthError with "Email is required" for blank email', () => {
    expect(() => authenticate('', 'correctPassword')).toThrow(AuthError);
    expect(() => authenticate('', 'correctPassword')).toThrow('Email is required');
  });

  it('throws AuthError when email consists only of whitespace', () => {
    expect(() => authenticate('   ', 'pass')).toThrow(AuthError);
    expect(() => authenticate('   ', 'pass')).toThrow('Email is required');
  });

  it('throws AuthError with "Password is required" for blank password', () => {
    expect(() => authenticate('a@b.com', '')).toThrow(AuthError);
    expect(() => authenticate('a@b.com', '')).toThrow('Password is required');
  });

  it('throws AuthError when password is null', () => {
    const nullPass: any = null;
    expect(() => authenticate('a@b.com', nullPass)).toThrow(AuthError);
    expect(() => authenticate('a@b.com', nullPass)).toThrow('Password is required');
  });

  it('throws AuthError with "Invalid credentials" for wrong password', () => {
    expect(() => authenticate('a@b.com', 'wrongOne')).toThrow(AuthError);
    expect(() => authenticate('a@b.com', 'wrongOne')).toThrow('Invalid credentials');
  });
});

describe('validateAge', () => {
  it('returns false for negative age', () => {
    expect(validateAge(-1)).toBe(false);
  });

  it('returns false for the smallest non-negative age', () => {
    expect(validateAge(0)).toBe(false);
  });

  it('returns false for age 17 just below the threshold', () => {
    expect(validateAge(17)).toBe(false);
  });

  it('returns true for age 18 at the threshold', () => {
    expect(validateAge(18)).toBe(true);
  });

  it('returns true for age 19 just above the threshold', () => {
    expect(validateAge(19)).toBe(true);
  });

  it('returns true for large age values', () => {
    expect(validateAge(100)).toBe(true);
  });
});

describe('createSession', () => {
  beforeEach(() => {
    jest.useFakeTimers();
    jest.setSystemTime(new Date('2024-01-01T00:00:00Z'));
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it('creates session with the provided user and a prefixed token', () => {
    const user = { id: 'user_42', email: 'a@b.com' };
    const session = createSession(user);

    expect(session.user).toStrictEqual(user);
    expect(session.token).toBe(`tok_user_42_${Date.now()}`);
  });

  it('computes expiresAt as exactly one hour from the current time', () => {
    const now = Date.now();
    const session = createSession({ id: 'user_99', email: 'a@b.com' });

    expect(session.expiresAt).toBe(now + 3_600_000);
  });

  it('embeds the user id in the generated token string', () => {
    const session = createSession({ id: 'user_7', email: 'a@b.com' });

    expect(session.token).toContain('user_7');
    expect(session.token).toMatch(/^tok_user_7_\d+$/);
  });
});

describe('AuthError', () => {
  it('can be thrown and caught with the correct name and message', () => {
    const thrower = () => { throw new AuthError('test message'); };

    expect(thrower).toThrow(AuthError);
    expect(thrower).toThrow('test message');

    const instance = new AuthError('another message');
    expect(instance.name).toBe('AuthError');
    expect(instance).toBeInstanceOf(Error);
  });
});
