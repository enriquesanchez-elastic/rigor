import { isValidEmail, parsePrice, clamp, ParseError } from '../validators';

describe('isValidEmail', () => {
  it('returns true for valid email formats and trims whitespace', () => {
    expect(isValidEmail('a@b.co')).toBe(true);
    expect(isValidEmail('x+y@z.io')).toBe(true);
    expect(isValidEmail('  a@b.co  ')).toBe(true);
  });

  it('returns false for null, undefined, and blank string', () => {
    const nullVal: any = null;
    const undefVal: any = undefined;
    expect(isValidEmail('')).toBe(false);
    expect(isValidEmail(nullVal)).toBe(false);
    expect(isValidEmail(undefVal)).toBe(false);
  });

  it('returns false for malformed addresses with missing parts', () => {
    expect(isValidEmail('no-at-sign')).toBe(false);
    expect(isValidEmail('user@')).toBe(false);
    expect(isValidEmail('@domain')).toBe(false);
    expect(isValidEmail('has spaces')).toBe(false);
  });
});

describe('parsePrice', () => {
  it('parses an integer string to the correct number', () => {
    expect(parsePrice('42')).toBe(42);
  });

  it('parses a decimal string to the correct number', () => {
    expect(parsePrice('9.99')).toBe(9.99);
  });

  it('parses the smallest valid non-negative value', () => {
    expect(parsePrice('0')).toBe(0);
  });

  it('trims surrounding whitespace before parsing', () => {
    expect(parsePrice('  100  ')).toBe(100);
  });

  it('throws ParseError with "Input is required" for null', () => {
    const nullVal: any = null;
    expect(() => parsePrice(nullVal)).toThrow(ParseError);
    expect(() => parsePrice(nullVal)).toThrow('Input is required');
  });

  it('throws ParseError with "Input is required" for undefined', () => {
    const undefVal: any = undefined;
    expect(() => parsePrice(undefVal)).toThrow(ParseError);
    expect(() => parsePrice(undefVal)).toThrow('Input is required');
  });

  it('throws ParseError for blank string input', () => {
    expect(() => parsePrice('')).toThrow(ParseError);
    expect(() => parsePrice('')).toThrow('Cannot parse empty string');
  });

  it('throws ParseError for whitespace-only input', () => {
    expect(() => parsePrice('   ')).toThrow(ParseError);
    expect(() => parsePrice('   ')).toThrow('Cannot parse empty string');
  });

  it('throws ParseError with the invalid token for non-numeric string', () => {
    expect(() => parsePrice('abc')).toThrow(ParseError);
    expect(() => parsePrice('abc')).toThrow('Invalid number: abc');
  });

  it('throws ParseError for mixed alphanumeric input', () => {
    expect(() => parsePrice('12abc')).toThrow(ParseError);
    expect(() => parsePrice('12abc')).toThrow('Invalid number: 12abc');
  });

  it('throws ParseError with "Price cannot be negative" for negative value', () => {
    expect(() => parsePrice('-5')).toThrow(ParseError);
    expect(() => parsePrice('-5')).toThrow('Price cannot be negative');
  });

  it('throws ParseError for small negative decimal', () => {
    expect(() => parsePrice('-0.01')).toThrow(ParseError);
    expect(() => parsePrice('-0.01')).toThrow('Price cannot be negative');
  });
});

describe('clamp', () => {
  it('returns the value when it falls within the range', () => {
    expect(clamp(5, 0, 10)).toBe(5);
  });

  it('returns the value at the exact lower limit', () => {
    expect(clamp(0, 0, 10)).toBe(0);
  });

  it('returns the value at the exact upper limit', () => {
    expect(clamp(10, 0, 10)).toBe(10);
  });

  it('clamps to min when value is just below the lower limit', () => {
    expect(clamp(-1, 0, 10)).toBe(0);
  });

  it('clamps to max when value is just above the upper limit', () => {
    expect(clamp(11, 0, 10)).toBe(10);
  });

  it('clamps extreme values far outside the range to the nearest limit', () => {
    expect(clamp(-1000, 0, 100)).toBe(0);
    expect(clamp(1000, 0, 100)).toBe(100);
  });

  it('handles negative ranges correctly', () => {
    expect(clamp(-5, -10, -1)).toBe(-5);
    expect(clamp(-15, -10, -1)).toBe(-10);
    expect(clamp(0, -10, -1)).toBe(-1);
  });
});

describe('ParseError', () => {
  it('can be thrown and caught with the correct name and message', () => {
    const thrower = () => { throw new ParseError('test parse failure'); };

    expect(thrower).toThrow(ParseError);
    expect(thrower).toThrow('test parse failure');

    const instance = new ParseError('another message');
    expect(instance.name).toBe('ParseError');
    expect(instance).toBeInstanceOf(Error);
  });
});
