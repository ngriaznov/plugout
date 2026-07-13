import { expect, test } from "@playwright/test";

test("scan renders the plugin table", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("row").nth(1)).toBeVisible();
  await expect(page.getByText(/plugins$/)).toBeVisible();
});

test("search narrows and clears", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/plugins$/)).toBeVisible();
  const search = page.getByLabel("Search plugins");
  await search.fill("zzzz-nothing");
  await expect(page.getByText(/no plugins match/i)).toBeVisible();

  await search.fill("");
  await expect(page.getByRole("row").nth(1)).toBeVisible();
  await expect(page.getByText(/plugins$/)).toBeVisible();
});

test("selection shows the action bar; removal confirm can be canceled", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/plugins$/)).toBeVisible();
  await page.getByRole("checkbox", { name: /^select (?!all)/i }).first().check();
  await page.getByRole("button", { name: /remove/i }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: /cancel/i }).click();
  await expect(dialog).not.toBeVisible();
});

test("removal flow completes with a toast", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/plugins$/)).toBeVisible();
  await page.getByRole("checkbox", { name: /^select (?!all)/i }).first().check();
  await page.getByRole("button", { name: /remove/i }).click();
  await page.getByRole("dialog").getByRole("button", { name: /trash/i }).click();
  await expect(page.getByText(/moved to Trash/)).toBeVisible();
});

test("export produces a toast", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/plugins$/)).toBeVisible();
  await page.getByRole("button", { name: /export/i }).click();
  await expect(page.getByText(/exported/i)).toBeVisible();
});

test("theme toggle flips the root theme", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/plugins$/)).toBeVisible();
  await page.getByRole("radio", { name: /dark/i }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", /dark/);
});
