describe('trivial assertions', () => {
  it('one equals one', () => {
    expect(1).toBe(1);
  });

  it('true is true', () => {
    expect(true).toBe(true);
  });

  it('literal string', () => {
    expect('hello').toBe('hello');
  });

  it('array identity', () => {
    const arr = [1, 2, 3];
    expect(arr).toEqual(arr);
  });

  it('two plus two', () => {
    expect(2 + 2).toBe(4);
  });
});
