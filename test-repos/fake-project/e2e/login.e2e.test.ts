import { test, expect } from '@playwright/test';

test('should redirect to dashboard on successful login', async ({ page }) => {
  await page.goto('/login');
  await page.fill('input[name="email"]', 'user@example.com');
  await page.fill('input[name="password"]', 'correctpassword');
  await page.click('button[type="submit"]');

  await expect(page).toHaveURL('/dashboard');
  await expect(page.locator('h1')).toHaveText('Welcome');
});

test('shows error for invalid credentials', async ({ page }) => {
  await page.goto('/login');
  await page.fill('input[name="email"]', 'user@example.com');
  await page.fill('input[name="password"]', 'wrong');
  await page.click('button[type="submit"]');

  const now = Date.now();
  await expect(page.locator('.error')).toBeVisible();
});

test('flaky: uses Date.now() for timing', async ({ page }) => {
  await page.goto('/login');
  const start = Date.now();
  await page.click('button[type="submit"]');
  const elapsed = Date.now() - start;
  expect(elapsed).toBeGreaterThanOrEqual(0);
});
