// Auth module for monorepo example
export class AuthService {
  private users: Map<string, { password: string; email: string }> = new Map();

  async register(email: string, password: string): Promise<{ id: string }> {
    if (!email || !email.includes('@')) {
      throw new Error('Invalid email');
    }
    if (!password || password.length < 8) {
      throw new Error('Password must be at least 8 characters');
    }
    
    const id = `user_${Date.now()}`;
    this.users.set(id, { password, email });
    return { id };
  }

  async login(email: string, password: string): Promise<{ token: string }> {
    for (const [id, user] of this.users.entries()) {
      if (user.email === email && user.password === password) {
        return { token: `token_${id}_${Date.now()}` };
      }
    }
    throw new Error('Invalid credentials');
  }

  async validateToken(token: string): Promise<boolean> {
    if (!token || token.length < 10) {
      return false;
    }
    return token.startsWith('token_');
  }
}
