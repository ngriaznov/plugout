import { expect, test } from "@playwright/test";

test("settings: usage toggle and scan locations add/remove persist", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /settings/i }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Folder path").fill("/tmp/my-plugins");
  await dialog.getByRole("button", { name: /^add$/i }).click();
  await expect(dialog.getByText("/tmp/my-plugins")).toBeVisible();
  await dialog.getByRole("button", { name: /remove \/tmp\/my-plugins/i }).click();
  await expect(dialog.getByText("/tmp/my-plugins")).not.toBeVisible();
});
