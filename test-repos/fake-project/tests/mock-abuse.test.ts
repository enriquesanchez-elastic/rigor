import { authenticate } from '../src/auth/auth';

jest.mock('fs');
jest.mock('path');
jest.mock('os');
jest.mock('crypto');
jest.mock('http');
jest.mock('https');

describe('mock abuse', () => {
  it('too many mocks', () => {
    const result = authenticate('user@example.com', 'correctPassword');
    expect(result.success).toBe(true);
  });
});

describe('mocking standard library', () => {
  beforeEach(() => {
    jest.spyOn(global, 'setTimeout').mockImplementation(((cb: () => void) => cb()) as typeof setTimeout);
  });

  it('mocked setTimeout', () => {
    let x = 0;
    setTimeout(() => {
      x = 1;
    }, 1000);
    expect(x).toBe(1);
  });
});
