// Example test file with weak assertions for demonstration
import { authenticate, validateUser, createSession } from './auth';

describe('Authentication', () => {
  let userData: any;

  it('should authenticate user', () => {
    const result = authenticate('user@example.com', 'password123');
    // Weak assertion - only checks if defined
    expect(result).toBeDefined();
  });

  it('should validate user data', () => {
    const user = { name: 'John', age: 25 };
    const isValid = validateUser(user);
    // Weak assertion - only checks truthiness
    expect(isValid).toBeTruthy();
  });

  it('should create session', () => {
    const session = createSession('user123');
    // Weak assertion - not.toBeNull is weak
    expect(session).not.toBeNull();
    expect(session.token).toBeDefined();
  });

  it('should handle empty input', () => {
    const result = authenticate('', '');
    // Weak assertion
    expect(result).toBeFalsy();
  });

  // Skipped test
  it.skip('should refresh token', () => {
    // Not implemented yet
  });

  // Test without assertions
  it('should log user activity', () => {
    console.log('Logging...');
    // No expect() calls
  });
});

describe('Session Management', () => {
  // Shared mutable state without beforeEach
  let sessions: string[] = [];

  it('step 1: creates first session', () => {
    sessions.push('session1');
    expect(sessions.length).toBe(1);
  });

  it('step 2: creates second session', () => {
    // This test depends on the previous one
    sessions.push('session2');
    expect(sessions.length).toBe(2);
  });
});
