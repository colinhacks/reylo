import { test } from "node:test";
import assert from "node:assert/strict";
import { canonical, normalized, sameMigration } from "../src/migration-text.ts";

// The port of `canonical` in engine/migrate/src/lock.rs: the Rust tests' cases, each read the same.
test("comments and spacing are no edit, as the lock reads them", () => {
  const a = "create table a (\n  id int -- the key\n);\n/* made by hand */";
  assert.equal(canonical(a), canonical("create table a ( id int );"));
  assert.notEqual(canonical("insert into a values ('-- not a comment')"), canonical("insert into a values ('')"));
  assert.equal(canonical("select $$ two  spaces $$"), "select $$ two  spaces $$");
  assert.equal(canonical("do $f$ begin -- x\n end $f$;"), "do $f$ begin -- x\n end $f$;");
  // MySQL's `\'` is a quote inside the string, so `-- b` is string text there, not a comment.
  assert.notEqual(canonical("insert into t values ('a\\' -- b')"), canonical("insert into t values ('a\\' -- c')"));
  assert.notEqual(canonical("select E'a\\', '-- b'"), canonical("select E'a\\', '-- c'"));
  assert.notEqual(canonical('insert into t values ("a\\" -- b")'), canonical('insert into t values ("a\\" -- c")'));
  // MySQL runs `/*! … */`, and reads `n--1` as `n - -1`.
  assert.notEqual(canonical("create table t (a int) /*!50100 PARTITION BY HASH (a) */"), canonical("create table t (a int) /*!50100 PARTITION BY KEY (a) */"));
  assert.notEqual(canonical("select 1--1"), canonical("select 1--2"));
  assert.equal(canonical("select 1 --\tx\n, 2 --\n"), "select 1 , 2");
  // A comment Postgres nests and MySQL ends at its first `*/`.
  assert.notEqual(canonical("/* a /* b */ create table t (a int); /* */"), canonical("/* a /* b */ create table t (a bigint); /* */"));
  assert.equal(canonical("select /* a /* b */ c */  1"), "select /* a /* b */ c */  1");
  assert.equal(canonical("select /* a */  1"), "select 1");
  // MySQL's `#` comments, and a comment that never ends.
  assert.notEqual(canonical("# see reylo/*.sql\ncreate table t (a int);"), canonical("# see reylo/*.sql\ncreate table t (a bigint);"));
  assert.notEqual(canonical("select 1; /* create table t (a int);"), canonical("select 1; /* create table t (a bigint);"));
  assert.equal(canonical("select 1 /* a */"), "select 1");
  // A backslash both readings agree on changes nothing.
  assert.equal(canonical("select 'a\\n' -- x\n,  'b\\\\'"), "select 'a\\n' , 'b\\\\'");
});

test("line endings and trailing whitespace are no edit, and a statement's change is one", () => {
  assert.equal(normalized("a\r\nb\rc  \n\n"), "a\nb\nc");
  assert.ok(sameMigration("create table t (\r\n  a int -- the key\r\n);\r\n", "create table t (\n  a int\n);"));
  assert.ok(sameMigration("insert into t values ('a\r\nb');", "insert into t values ('a\nb');"));
  assert.ok(!sameMigration("create table t (a int);", "create table t (a bigint);"));
});
