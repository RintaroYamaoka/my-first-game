import { expect, test } from "vitest";
import { parse } from "../src/parse";

test("parses a fragment id", () => {
  expect(parse("/fragment/f-0042")).toBe("f-0042");
});

test("rejects anything else", () => {
  expect(parse("/fragment/")).toBeNull();
  expect(parse(undefined)).toBeNull();
});
