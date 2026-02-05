import { isValidEmail, parsePrice, clamp } from '../utils/validators';

describe('validators', () => {
  it('isValidEmail', () => {
    expect(isValidEmail('user@example.com')).toMatchSnapshot();
    expect(isValidEmail('invalid')).toMatchSnapshot();
    expect(isValidEmail('')).toMatchSnapshot();
  });

  it('parsePrice', () => {
    expect(parsePrice('42.5')).toMatchSnapshot();
    expect(parsePrice('0')).toMatchSnapshot();
  });

  it('clamp', () => {
    expect(clamp(5, 0, 10)).toMatchSnapshot();
    expect(clamp(-1, 0, 10)).toMatchSnapshot();
    expect(clamp(15, 0, 10)).toMatchSnapshot();
  });
});
