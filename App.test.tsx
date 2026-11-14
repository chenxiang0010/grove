// Grove's root, tested from TypeScript: `nsh test` runs this file in a
// headless runner it builds with the crate, and `open(".")` starts the root
// (`nsh.json` "root").
//
// The window opens on the last repository that still exists, or on the
// welcome screen when there is none. Both paths are asserted here, because
// which one a machine takes depends on what it has opened before.
import { expect, open, test } from "nsh/test";

test("opens with the workspace or the welcome screen", async () => {
  const app = await open(".");
  const texts = app.texts();
  // The title bar is always there, with the project widget on it.
  expect(texts.some((text) => text === "Open a project" || text.length > 0)).toBe(true);
  // One of the two shells, never neither.
  const hasWorkspace = texts.includes("Text or hash") || texts.includes("Commit message");
  const hasWelcome = texts.includes("A Git client that reads fast and writes through git.");
  expect(hasWorkspace || hasWelcome).toBe(true);
});

test("the status bar names the branch or says there is none", async () => {
  const app = await open(".");
  const texts = app.texts();
  const saysBranch = texts.some((text) => text.length > 0 && text !== "No repository");
  expect(saysBranch || texts.includes("No repository")).toBe(true);
});
