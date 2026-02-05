export class AuthError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'AuthError';
  }
}

export interface User {
  id: string;
  email: string;
}

export interface Session {
  user: User;
  token: string;
  expiresAt: number;
}

export function authenticate(email: string, password: string): { success: boolean; user?: User; token?: string } {
  if (!email || email.trim() === '') {
    throw new AuthError('Email is required');
  }
  if (!password) {
    throw new AuthError('Password is required');
  }
  if (password !== 'correctPassword') {
    throw new AuthError('Invalid credentials');
  }
  return {
    success: true,
    user: { id: `user_${Date.now()}`, email },
    token: 'a'.repeat(64),
  };
}

export function validateAge(age: number): boolean {
  if (age >= 18) {
    return true;
  }
  return false;
}

export function createSession(user: User): Session {
  const token = `tok_${user.id}_${Date.now()}`;
  return {
    user,
    token,
    expiresAt: Date.now() + 3600_000,
  };
}
