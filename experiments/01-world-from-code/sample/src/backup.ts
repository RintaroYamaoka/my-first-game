import { writeFileSync } from "node:fs";
import { load } from "./config";
import { log } from "./log";
import { store } from "./store";

export function backup(): string {
  const cfg = load();
  const name = `${cfg.dataFile}.${Date.now()}.bak`;
  writeFileSync(name, JSON.stringify(store));
  log(`backup ${name}`);
  return name;
}
