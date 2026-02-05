import { isValidEmail, parsePrice, clamp } from '../src/utils/validators';

describe('snapshot only - no real assertions', () => {
  it('email snapshots', () => {
    expect(isValidEmail('a@b.com')).toMatchSnapshot();
    expect(isValidEmail('x')).toMatchSnapshot();
    expect(isValidEmail('')).toMatchSnapshot();
  });

  it('price snapshots', () => {
    expect(parsePrice('0')).toMatchSnapshot();
    expect(parsePrice('42.5')).toMatchSnapshot();
  });

  it('clamp snapshots', () => {
    expect(clamp(0, 0, 10)).toMatchSnapshot();
    expect(clamp(10, 0, 10)).toMatchSnapshot();
    expect(clamp(5, 0, 10)).toMatchSnapshot();
  });

  it('inline snapshot', () => {
    expect({ a: 1, b: 'x' }).toMatchInlineSnapshot(`
      Object {
        "a": 1,
        "b": "x",
      }
    `);
  });
});
