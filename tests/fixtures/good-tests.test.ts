// Example test file demonstrating good testing practices
import { authenticate, validateUser, createSession, AuthError } from './auth';

describe('Authentication', () => {
  describe('authenticate', () => {
    it('should return user object with correct email on successful login', () => {
      const result = authenticate('user@example.com', 'correctPassword');

      expect(result.success).toBe(true);
      expect(result.user.email).toBe('user@example.com');
      expect(result.user.id).toMatch(/^user_[a-z0-9]+$/);
    });

    it('should return token with valid expiration time', () => {
      const result = authenticate('user@example.com', 'correctPassword');

      expect(result.token).toHaveLength(64);
      expect(result.expiresAt).toBeGreaterThan(Date.now());
      expect(result.expiresAt).toBeLessThan(Date.now() + 86400000);
    });

    it('should throw AuthError for invalid credentials', () => {
      expect(() => authenticate('user@example.com', 'wrongPassword')).toThrow(AuthError);
      expect(() => authenticate('user@example.com', 'wrongPassword')).toThrow('Invalid credentials');
    });

    it('should throw AuthError for empty email', () => {
      expect(() => authenticate('', 'password')).toThrow('Email is required');
    });

    it('should throw AuthError for empty password', () => {
      expect(() => authenticate('user@example.com', '')).toThrow('Password is required');
    });
  });

  describe('validateUser', () => {
    it('should return true for valid user with all required fields', () => {
      const user = { name: 'John Doe', email: 'john@example.com', age: 25 };
      expect(validateUser(user)).toBe(true);
    });

    it('should return false for user with missing name', () => {
      const user = { email: 'john@example.com', age: 25 };
      expect(validateUser(user)).toBe(false);
    });

    it('should return false for user with age below minimum (18)', () => {
      const user = { name: 'John', email: 'john@example.com', age: 17 };
      expect(validateUser(user)).toBe(false);
    });

    it('should return true for user at minimum age boundary (18)', () => {
      const user = { name: 'John', email: 'john@example.com', age: 18 };
      expect(validateUser(user)).toBe(true);
    });

    it('should return true for user above minimum age (19)', () => {
      const user = { name: 'John', email: 'john@example.com', age: 19 };
      expect(validateUser(user)).toBe(true);
    });

    it('should handle null input gracefully', () => {
      expect(validateUser(null)).toBe(false);
    });

    it('should handle undefined input gracefully', () => {
      expect(validateUser(undefined)).toBe(false);
    });

    it('should handle empty object', () => {
      expect(validateUser({})).toBe(false);
    });

    it('should handle user with age 0', () => {
      const user = { name: 'Baby', email: 'baby@example.com', age: 0 };
      expect(validateUser(user)).toBe(false);
    });

    it('should handle user with negative age', () => {
      const user = { name: 'Invalid', email: 'invalid@example.com', age: -1 };
      expect(validateUser(user)).toBe(false);
    });
  });

  describe('createSession', () => {
    let mockDate: jest.SpyInstance;

    beforeEach(() => {
      mockDate = jest.spyOn(Date, 'now').mockReturnValue(1000000);
    });

    afterEach(() => {
      mockDate.mockRestore();
    });

    it('should create session with correct user ID', () => {
      const session = createSession('user123');
      expect(session.userId).toBe('user123');
    });

    it('should generate unique session token', () => {
      const session1 = createSession('user1');
      const session2 = createSession('user2');
      expect(session1.token).not.toBe(session2.token);
    });

    it('should set correct creation timestamp', () => {
      const session = createSession('user123');
      expect(session.createdAt).toBe(1000000);
    });

    it('should throw error for empty user ID', () => {
      expect(() => createSession('')).toThrow('User ID is required');
    });
  });
});
