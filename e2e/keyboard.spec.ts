import { expect, test } from "@playwright/test";

test("slash focuses search; arrows traverse rows; Space selects; Enter inspects", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/plugins$/)).toBeVisible();
  await page.keyboard.press("/");
  await expect(page.locator("#plugin-search")).toBeFocused();
  await page.keyboard.press("Escape");
  await page.locator("tr[tabindex='0']").focus();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press(" ");
  await expect(page.getByRole("button", { name: /remove/i })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.locator(".inspector")).toBeVisible();
});

test("confirm modal traps focus", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/plugins$/)).toBeVisible();
  await page.getByRole("checkbox").nth(1).check();
  await page.getByRole("button", { name: /remove/i }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  for (let i = 0; i < 10; i++) await page.keyboard.press("Tab");
  const focusInDialog = await dialog.evaluate((el) => el.contains(document.activeElement));
  expect(focusInDialog).toBe(true);
});
