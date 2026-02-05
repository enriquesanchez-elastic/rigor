import { test, expect } from '@playwright/test';

test('uses Date.now for timing', async ({ page }) => {
  const t0 = Date.now();
  await page.goto('/');
  await page.click('button');
  expect(Date.now() - t0).toBeGreaterThanOrEqual(0);
});

test('uses Math.random', async ({ page }) => {
  await page.goto('/');
  const seed = Math.random();
  expect(seed).toBeLessThanOrEqual(1);
});

test('setTimeout in test', async ({ page }) => {
  let done = false;
  setTimeout(() => {
    done = true;
  }, 100);
  await page.goto('/');
  expect(done).toBe(false);
});
