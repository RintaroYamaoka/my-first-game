import { z } from "zod";

const Id = z.string().regex(/^f-\d{4}$/);

export function parse(url: string | undefined): string | null {
  const m = /^\/fragment\/([^/]+)$/.exec(url ?? "");
  if (!m) return null;
  const id = Id.safeParse(m[1]);
  return id.success ? id.data : null;
}
