// Exact-type pins for the lab: assignable both ways means the inferred type equals the expected one.
import getUser from "./get_user.sql";
import type { Row, Params } from "./get_user.sql";
import { sql } from "./db";

type ExpectedRow = {
  id: number;
  name: string;
  email: string | null;
  role: "admin" | "member";
  tags: (string | null)[];
  created_at: string;
  big: bigint | null;
};
export const a: ExpectedRow = null! as Row;
export const b: Row = null! as ExpectedRow;
export const c: { id: number } = null! as Params;
export const d: Params = null! as { id: number };
export async function pins() {
  const row: Row | null = await getUser.run(sql, { id: 1 });
  // @ts-expect-error wrong parameter type
  await getUser.run(sql, { id: "1" });
  return row;
}
