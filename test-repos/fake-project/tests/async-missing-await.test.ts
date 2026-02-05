describe('missing await on promises', () => {
  function asyncValue(): Promise<number> {
    return Promise.resolve(42);
  }

  it('resolves without await', () => {
    expect(asyncValue().then((x) => x)).resolves.toBe(42);
  });

  it('rejects without await', () => {
    expect(Promise.reject(new Error('fail'))).rejects.toThrow('fail');
  });

  it('async test with no await', async () => {
    const x = asyncValue();
    expect(x).toBeDefined();
  });
});
