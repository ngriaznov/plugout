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
