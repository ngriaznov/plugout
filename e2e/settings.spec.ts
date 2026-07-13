import { expect, test } from "@playwright/test";

test("settings: usage toggle persists across reopen", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /settings/i }).click();
  const dialog = page.getByRole("dialog");
  const usageToggle = dialog.getByRole("checkbox", { name: /scan daw projects/i });
  await expect(usageToggle).not.toBeChecked();
  await usageToggle.check();
  await expect(usageToggle).toBeChecked();

  await dialog.getByRole("button", { name: /close settings/i }).click();
  await expect(dialog).not.toBeVisible();

  await page.getByRole("button", { name: /settings/i }).click();
  await expect(page.getByRole("dialog").getByRole("checkbox", { name: /scan daw projects/i })).toBeChecked();
});

test("settings: scan locations add/remove persist", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /settings/i }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Folder path").fill("/tmp/my-plugins");
  await dialog.getByRole("button", { name: /^add$/i }).click();
  await expect(dialog.getByText("/tmp/my-plugins")).toBeVisible();
  await dialog.getByRole("button", { name: /remove \/tmp\/my-plugins/i }).click();
  await expect(dialog.getByText("/tmp/my-plugins")).not.toBeVisible();
});

test("settings: escape-close rescans when scan locations changed", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/plugins$/)).toBeVisible();
  await page.getByRole("button", { name: /settings/i }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Folder path").fill("/tmp/escape-rescan-plugins");
  await dialog.getByRole("button", { name: /^add$/i }).click();
  await expect(dialog.getByText("/tmp/escape-rescan-plugins")).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();

  await expect(page.locator(".count").getByText(/scanning…/i)).toBeVisible();
  await expect(page.getByText(/plugins$/)).toBeVisible();
});
