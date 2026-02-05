import { authenticate, validateAge, createSession, AuthError } from '../src/auth/auth';

describe('Authentication', () => {
  it('should return user object with correct email on successful login', () => {
    const result = authenticate('user@example.com', 'correctPassword');

    expect(result.success).toBe(true);
    expect(result.user).toBeDefined();
    expect(result.user!.email).toBe('user@example.com');
    expect(result.user!.id).toMatch(/^user_\d+$/);
    expect(result.token).toHaveLength(64);
  });

  it('should throw AuthError for invalid credentials', () => {
    expect(() => authenticate('user@example.com', 'wrongPassword')).toThrow(AuthError);
    expect(() => authenticate('user@example.com', 'wrongPassword')).toThrow('Invalid credentials');
  });

  it('should throw AuthError for empty email', () => {
    expect(() => authenticate('', 'password')).toThrow(AuthError);
    expect(() => authenticate('', 'password')).toThrow('Email is required');
  });

  it('should throw AuthError for null password', () => {
    expect(() => authenticate('user@example.com', '')).toThrow(AuthError);
    expect(() => authenticate('user@example.com', null as unknown as string)).toThrow('Password is required');
  });

  describe('validateAge boundary', () => {
    it('returns false for age 17', () => {
      expect(validateAge(17)).toBe(false);
    });

    it('returns true for age 18', () => {
      expect(validateAge(18)).toBe(true);
    });

    it('returns true for age 19', () => {
      expect(validateAge(19)).toBe(true);
    });
  });

  describe('createSession', () => {
    beforeEach(() => {
      jest.useFakeTimers();
    });

    it('returns session with user and token', () => {
      const user = { id: 'user_1', email: 'a@b.com' };
      const session = createSession(user);

      expect(session.user).toEqual(user);
      expect(session.token).toMatch(/^tok_user_1_\d+$/);
      expect(session.expiresAt).toBeGreaterThan(Date.now());
    });
  });
});
