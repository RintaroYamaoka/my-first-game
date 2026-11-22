import { appendFileSync } from "node:fs";
import { load } from "./config";

// 循環の見本: log → config → parse → log と、頼り合って輪になっている
export function log(line: string): void {
  const stamp = new Date().toISOString();
  appendFileSync(load().logFile ?? "world.log", `${stamp} ${line}\n`);
}
