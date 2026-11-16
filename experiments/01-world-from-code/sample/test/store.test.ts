import { expect, test } from "vitest";
import { find, put } from "../src/store";

test("finds what was put", () => {
  put({ id: "f-0042", body: "first light", at: 0 });
  expect(find("f-0042")?.body).toBe("first light");
});
