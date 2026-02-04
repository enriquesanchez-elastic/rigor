// Test file for monorepo auth package
import { AuthService } from '../src/auth';

describe('AuthService', () => {
  let authService: AuthService;

  beforeEach(() => {
    authService = new AuthService();
  });

  describe('register', () => {
    it('should register a user with valid email and password', async () => {
      const result = await authService.register('test@example.com', 'password123');
      
      expect(result).toBeDefined();
      expect(result.id).toMatch(/^user_\d+$/);
    });

    it('should throw error for invalid email', async () => {
      await expect(authService.register('invalid', 'password123'))
        .rejects.toThrow('Invalid email');
    });

    it('should throw error for short password', async () => {
      await expect(authService.register('test@example.com', 'short'))
        .rejects.toThrow('Password must be at least 8 characters');
    });
  });

  describe('login', () => {
    beforeEach(async () => {
      await authService.register('user@example.com', 'correctpassword');
    });

    it('should return token for valid credentials', async () => {
      const result = await authService.login('user@example.com', 'correctpassword');
      
      expect(result.token).toBeDefined();
      expect(result.token).toMatch(/^token_user_/);
    });

    it('should throw error for invalid credentials', async () => {
      await expect(authService.login('user@example.com', 'wrongpassword'))
        .rejects.toThrow('Invalid credentials');
    });
  });

  describe('validateToken', () => {
    it('should return true for valid token format', async () => {
      const isValid = await authService.validateToken('token_user_123456789');
      expect(isValid).toBe(true);
    });

    it('should return false for invalid token', async () => {
      const isValid = await authService.validateToken('invalid');
      expect(isValid).toBe(false);
    });

    it('should return false for empty token', async () => {
      const isValid = await authService.validateToken('');
      expect(isValid).toBe(false);
    });
  });
});
