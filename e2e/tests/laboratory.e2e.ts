import { test } from "@e2e-dev/web";
import { expect } from "e2e";

const user = process.env.CYBEREPI_USER ?? "admin";
const password = process.env.CYBEREPI_PASSWORD ?? "cyberepi-dev";

test("laboratory walkthrough", async ({ app, screen }) => {
  await app.open("/");
  await expect(screen.getByRole("heading", "Welcome to CyberEpi")).toBeVisible();
  await app.screenshot("01-welcome");

  await screen.getByRole("textbox", "Name").fill(user);
  await screen.getByRole("textbox", "Password").fill(password);
  await screen.getByRole("button", "Enter laboratory").tap();
  await expect(screen.getByRole("button", "Run demo experiment")).toBeVisible();
  await app.screenshot("02-ready");

  await screen.getByRole("button", "Run demo experiment").tap();
  await expect(screen.getByText("Attack rate")).toBeVisible({ timeout: 180_000 });
  await new Promise((resolve) => setTimeout(resolve, 2000));
  await app.screenshot("03-experiment");

  await app.open("/scenarios/new");
  await expect(screen.getByRole("heading", "Scenario")).toBeVisible();
  await app.screenshot("04-scenario");
});
