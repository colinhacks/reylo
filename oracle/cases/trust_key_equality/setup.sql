-- A unique key pinned with `=` only where that `=` is the key's own equality. A domain's `=` of a
-- user's, true for anything, is what a literal beside a column of the domain resolves to, and no
-- unique index compares with it. citext's `=` is the btree equality of citext_ops, under which its
-- unique keys are built. A built-in key beside both still pins, and so does an enum's, whose `=` is
-- the built-in one of every enum, beside a user's `=` of other types.
create domain mytext as text;
create function mytext_true(mytext, mytext) returns bool language sql immutable as $$ select true $$;
create operator = (leftarg = mytext, rightarg = mytext, function = mytext_true);
create table u (d mytext primary key, n text not null);
create extension citext;
create table c (h citext primary key, n text not null);
create table b (id bigint primary key, n text not null);
create type mood as enum ('calm', 'busy');
create table e (m mood primary key, n text not null);
-- A key compared with an integer sum, the wider integer, beside a user's `=`; a call of a string
-- beside a text key proves nothing here, since a domain of text has a `=` of its own. A view
-- passes a table's key through, under the same equality as the table's own, a domain's included.
create view cv as select h, n from c;
create view uv as select d, n from u;
