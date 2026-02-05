const EMAIL_REGEX = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function isValidEmail(email: string): boolean {
  if (!email || typeof email !== 'string') return false;
  return EMAIL_REGEX.test(email.trim());
}

export class ParseError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ParseError';
  }
}

export function parsePrice(str: string): number {
  if (str === null || str === undefined) {
    throw new ParseError('Input is required');
  }
  const trimmed = String(str).trim();
  if (trimmed === '') {
    throw new ParseError('Cannot parse empty string');
  }
  const num = Number(trimmed);
  if (Number.isNaN(num)) {
    throw new ParseError(`Invalid number: ${trimmed}`);
  }
  if (num < 0) {
    throw new ParseError('Price cannot be negative');
  }
  return num;
}

export function clamp(value: number, min: number, max: number): number {
  if (value < min) return min;
  if (value > max) return max;
  return value;
}
