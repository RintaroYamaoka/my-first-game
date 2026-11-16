import { appendFileSync } from "node:fs";

export function log(line: string): void {
  const stamp = new Date().toISOString();
  appendFileSync("world.log", `${stamp} ${line}\n`);
}
