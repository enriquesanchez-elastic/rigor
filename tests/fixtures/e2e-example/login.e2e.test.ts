// E2E test example - no direct source file mapping
import { test, expect, Page } from '@playwright/test';

test.describe('Login Flow', () => {
  let page: Page;

  test.beforeEach(async ({ browser }) => {
    page = await browser.newPage();
    await page.goto('/login');
  });

  test.afterEach(async () => {
    await page.close();
  });

  test('should display login form', async () => {
    await expect(page.locator('form[data-testid="login-form"]')).toBeVisible();
    await expect(page.locator('input[name="email"]')).toBeVisible();
    await expect(page.locator('input[name="password"]')).toBeVisible();
    await expect(page.locator('button[type="submit"]')).toBeVisible();
  });

  test('should show error for invalid credentials', async () => {
    await page.fill('input[name="email"]', 'invalid@example.com');
    await page.fill('input[name="password"]', 'wrongpassword');
    await page.click('button[type="submit"]');

    await expect(page.locator('[data-testid="error-message"]')).toBeVisible();
    await expect(page.locator('[data-testid="error-message"]')).toHaveText('Invalid credentials');
  });

  test('should redirect to dashboard on successful login', async () => {
    await page.fill('input[name="email"]', 'user@example.com');
    await page.fill('input[name="password"]', 'correctpassword');
    await page.click('button[type="submit"]');

    await expect(page).toHaveURL('/dashboard');
    await expect(page.locator('h1')).toHaveText('Welcome');
  });

  test('should show password field as masked', async () => {
    const passwordInput = page.locator('input[name="password"]');
    await expect(passwordInput).toHaveAttribute('type', 'password');
  });

  test('should have remember me checkbox', async () => {
    await expect(page.locator('input[name="remember"]')).toBeVisible();
    await expect(page.locator('input[name="remember"]')).not.toBeChecked();
  });

  test('should navigate to forgot password page', async () => {
    await page.click('a[href="/forgot-password"]');
    await expect(page).toHaveURL('/forgot-password');
  });

  test('should validate email format', async () => {
    await page.fill('input[name="email"]', 'notanemail');
    await page.fill('input[name="password"]', 'somepassword');
    await page.click('button[type="submit"]');

    await expect(page.locator('[data-testid="email-error"]')).toBeVisible();
  });
});
