import { test } from "node:test";
import assert from "node:assert/strict";
import { DatabaseSync } from "node:sqlite";
import { withSqlcommenter } from "../src/sqlcommenter.ts";

test("the comment ends the statement, before a closing semicolon, its file URL-encoded", () => {
  const tag = "/*file='queries%2Fget_user.sql'*/";
  assert.equal(withSqlcommenter("select 1", "queries/get_user.sql"), `select 1 ${tag}`);
  assert.equal(withSqlcommenter("select 1\n", "queries/get_user.sql"), `select 1 ${tag}`);
  assert.equal(withSqlcommenter("select 1;\n", "queries/get_user.sql"), `select 1 ${tag};`);
  assert.equal(withSqlcommenter("select 1 ;  \n\n", "queries/get_user.sql"), `select 1 ${tag};`);
  // After a line comment the tag starts a line of its own, so the comment does not take it in.
  assert.equal(withSqlcommenter("select 1 -- the one\n", "queries/get_user.sql"), `select 1 -- the one\n${tag}`);
  assert.equal(withSqlcommenter("select 1 # MySQL's\n", "queries/get_user.sql"), `select 1 # MySQL's\n${tag}`);
  // A quote, which URL encoding leaves, is escaped; a space, a `?` or a `:` is encoded, so no
  // placeholder can appear in the comment.
  assert.equal(withSqlcommenter("select 1", "it's here/a b?:c.sql"), "select 1 /*file='it\\'s%20here%2Fa%20b%3F%3Ac.sql'*/");
});

test("each form runs on SQLite, its comment ignored", () => {
  const db = new DatabaseSync(":memory:");
  for (const text of ["select 1 as n", "select 1 as n;\n", "select 1 as n -- the one\n", "select 1 as n /* already */\n"]) {
    assert.deepEqual({ ...db.prepare(withSqlcommenter(text, "queries/one.sql")).get() }, { n: 1 }, text);
  }
});
