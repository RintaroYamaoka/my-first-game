import { readFileSync } from "node:fs";
import { z } from "zod";

const Config = z.object({
  port: z.number().int().min(1).max(65535),
  dataFile: z.string(),
});

export type Config = z.infer<typeof Config>;

export function load(path = "config.json"): Config {
  const raw = JSON.parse(readFileSync(path, "utf8"));
  return Config.parse(raw);
}
