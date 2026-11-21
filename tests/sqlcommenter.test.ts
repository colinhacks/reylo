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

test("on SQLite the comment opens the statement, as it is written", () => {
  const tag = "/*file='queries%2Fget_user.sql'*/";
  assert.equal(withSqlcommenter("select 1", "queries/get_user.sql", "sqlite"), `${tag} select 1`);
  assert.equal(withSqlcommenter("select 1;\n", "queries/get_user.sql", "sqlite"), `${tag} select 1;\n`);
  assert.equal(withSqlcommenter("-- the one\nselect 1", "queries/get_user.sql", "sqlite"), `${tag} -- the one\nselect 1`);
});

test("each form runs on SQLite, its comment ignored and in no column's name", () => {
  const db = new DatabaseSync(":memory:");
  const name = (text: string) => Object.keys(db.prepare(text).get() as object);
  for (const text of ["select 1 as n", "select 1 as n;\n", "select 1 as n -- the one\n", "select 1 as n /* already */\n"]) {
    assert.deepEqual(name(withSqlcommenter(text, "queries/one.sql", "sqlite")), ["n"], text);
  }
  for (const text of ["select 1 + 1", "select 1 + 1;\n", "select 1 +\n  1\n"]) {
    assert.deepEqual(name(withSqlcommenter(text, "queries/one.sql", "sqlite")), name(text), text);
  }
  // What the comment does at the end, and why it is not there.
  assert.deepEqual(name(withSqlcommenter("select 1 + 1", "one.sql")), ["1 + 1 /*file='one.sql'*/"]);
});
