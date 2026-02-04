// Sample source file with functions that can throw and have boundary conditions

export class AuthError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'AuthError';
  }
}

export interface User {
  id: string;
  email: string;
  name?: string;
  age?: number;
}

export interface AuthResult {
  success: boolean;
  user: User;
  token: string;
  expiresAt: number;
}

export interface Session {
  userId: string;
  token: string;
  createdAt: number;
}

/**
 * Authenticate a user with email and password
 */
export function authenticate(email: string, password: string): AuthResult {
  if (!email) {
    throw new AuthError('Email is required');
  }

  if (!password) {
    throw new AuthError('Password is required');
  }

  // Simulated authentication logic
  if (password !== 'correctPassword') {
    throw new AuthError('Invalid credentials');
  }

  return {
    success: true,
    user: {
      id: `user_${Math.random().toString(36).substring(7)}`,
      email: email,
    },
    token: generateToken(64),
    expiresAt: Date.now() + 3600000, // 1 hour
  };
}

/**
 * Validate user data
 */
export function validateUser(user: any): boolean {
  if (!user) {
    return false;
  }

  if (!user.name || typeof user.name !== 'string') {
    return false;
  }

  if (!user.email || typeof user.email !== 'string') {
    return false;
  }

  // Age validation with boundary condition: must be >= 18
  if (typeof user.age !== 'number' || user.age < 18) {
    return false;
  }

  return true;
}

/**
 * Create a new session for a user
 */
export function createSession(userId: string): Session {
  if (!userId) {
    throw new Error('User ID is required');
  }

  return {
    userId,
    token: generateToken(32),
    createdAt: Date.now(),
  };
}

/**
 * Check if a value is within allowed range
 */
export function isInRange(value: number, min: number, max: number): boolean {
  if (value < min) {
    return false;
  }
  if (value > max) {
    return false;
  }
  return true;
}

/**
 * Calculate discount based on quantity
 */
export function calculateDiscount(quantity: number): number {
  if (quantity <= 0) {
    throw new Error('Quantity must be positive');
  }

  // Boundary conditions for discount tiers
  if (quantity >= 100) {
    return 0.20; // 20% discount
  } else if (quantity >= 50) {
    return 0.15; // 15% discount
  } else if (quantity >= 10) {
    return 0.10; // 10% discount
  }

  return 0;
}

function generateToken(length: number): string {
  const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
  let token = '';
  for (let i = 0; i < length; i++) {
    token += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return token;
}
