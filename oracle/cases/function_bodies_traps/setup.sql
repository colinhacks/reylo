-- Each statement here once made a reading of a function's body claim what the database contradicts.
create table comments (id bigint primary key, post_id bigint not null, body text not null, note text);
create table posts (id bigint primary key, title text not null, archived boolean not null default false, done boolean not null default false, subtitle text);
create table codes (id bigint primary key, code char(6) not null, bits varbit(8) not null);
insert into comments values (1, 1, 'b1', null), (2, 1, 'b2', 'n'), (3, 2, 'b3', null);
insert into posts values (1, 't', false, false, null), (2, 'u', true, false, 's');
insert into codes values (1, 'abcdef', B'1010');

-- A function that returns one value gives that value for its whole row, where a derived table gives a row.
create function bodies_for(p bigint) returns table (body text) language sql stable as $$ select body from comments where post_id = p $$;
create function single_out(x bigint, out a bigint) language sql stable as $$ select x $$;
create function many_bodies(p bigint) returns setof text language sql stable as $$ select body from comments where post_id = p $$;

-- A FROM item with no alias still has columns a name could mean.
create function unaliased(x text) returns table (v text) language sql stable as $$ select x from (select 'lit'::text as note) $$;

-- A type written with no length means another length in a cast.
create function echo_char(x char) returns table (y char) language sql stable as $$ select x $$;
create function chars() returns table (c char) language sql stable as $$ select code from codes $$;
create function bit_values() returns table (b bit) language sql stable as $$ select bits from codes $$;

-- A composite of one field is the row, not the field.
create type one_col as (a int);
create function wrap0() returns one_col language sql stable as $$ select 1 $$;

-- A search path of a function reaches every function it calls, at every level.
create schema s1;
create table s1.comments (id bigint primary key, post_id bigint not null, body text);
insert into s1.comments values (1, 1, null);
create function g_inner() returns table (b text) language sql stable as $$ select body from comments $$;
create function f_path() returns table (b text) language sql stable set search_path = s1 as $$ select b from public.g_inner() $$;
create function h_string() returns table (b text) language sql stable as $$ select body from comments $$;
create function g_atomic() returns table (b text) language sql stable begin atomic select b from public.h_string(); end;
create function f_path_atomic() returns table (b text) language sql stable set search_path = s1 as $$ select b from public.g_atomic() $$;
create function path_cte() returns table (body text) language sql stable set search_path = s1 as $$ with a as (select body from comments), comments as (select 1) select body from a $$;

-- A strict function's test of its argument must not read the function's own output.
create table has_a (a bigint);
insert into has_a values (null), (1);
create function strict_null(p bigint) returns table (a text) language sql stable strict as $$ select null::text $$;
create function strict_row(p bigint) returns table (a text) language sql stable strict as $$ select 'x' $$;

-- A constant in an ORDER BY is a position.
create function one() returns int language sql stable as $$ select 1 $$;

-- A call of one argument on a row may be its column.
create table priced (id int primary key, price int);
insert into priced values (1, null);
create function price(x int) returns int language sql stable as $$ select 5 $$;

-- A setting that changes how a body parses.
create table u (id bigint primary key, code text unique);
insert into u values (1, null), (2, null), (3, 'a');
create function by_null_eq(x text) returns setof u language sql stable set transform_null_equals = on as $$ select * from u where code = null $$;

-- An alias's column list renames what a whole row's field reads.
create table flags (a boolean, b boolean not null);
insert into flags values (null, true);
create function field_a(r flags) returns boolean language sql stable as $$ select r.a $$;

-- A user's cast can return NULL for a value.
create type wrapped as (v int);
create function int_to_wrapped(int) returns wrapped language sql immutable as $$ select null::wrapped $$;
create cast (int as wrapped) with function int_to_wrapped(int) as implicit;
create function strict_wrapped(w wrapped) returns text language sql stable strict as $$ select 'x' $$;
create function strict_wrapped_set(w wrapped) returns table (a text) language sql stable strict as $$ select 'x' $$;

-- A VALUES list's columns go by column1 and on.
create function is_open(t posts) returns boolean language sql stable as $$ select not t.archived and not t.done $$;

-- The server prints a parameter of a body stored parsed under the function's name.
create function atomic_alias(note text) returns table (v text) language sql stable begin atomic select $1 from comments atomic_alias where note is not null; end;

-- A subquery of a body shows one value where a call's changes from row to row.
create function pick(x bigint) returns bigint language sql stable as $$ select x from (select 1) s $$;
create function lit(x bigint) returns table (v bigint) language sql stable as $$ select x $$;
create function one_comment(x bigint) returns table (id bigint, body text) language sql stable as $$ select id, body from comments where id = x $$;

-- A strict function reached through a body, whose argument may be null there.
create function g_strict(x text) returns text language sql stable strict as $$ select 'x' $$;
create function f_through(x text) returns jsonb language sql stable as $$ select jsonb_build_object('a', g_strict(x)) $$;
create function g_strict_set(x text) returns table (v text) language sql stable strict as $$ select 'x' $$;
create function f_through_set(x text) returns table (v text) language sql stable as $$ select v from g_strict_set(x) $$;

-- An argument's collation is the function's, which a parameter standing for it does not carry.
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
create table ci_tab (id int primary key, c text collate ci not null);
create table ci_keys (id int primary key, code text unique);
insert into ci_tab values (1, 'a');
insert into ci_keys values (1, 'a'), (2, 'A');
create function by_code(x text) returns table (id int) language sql stable as $$ select id from ci_keys where code = x $$;
-- A call has one collation, for all it takes: a column among its arguments gives it to a constant beside it.
create function by_two(x text, y text) returns table (id int) language sql stable as $$ select id from ci_keys where code = x and y is not null $$;
-- A cast of a cast may turn a value to NULL.
create function strict_i(x bigint) returns text language sql stable strict as $$ select 'x' $$;
create function strict_iset(x bigint) returns table (a text) language sql stable strict as $$ select 'x' $$;

-- One output of a row type returns that row, whose whole row is its columns, not a row of one column.
create function row_col(p bigint) returns table (r comments) language sql stable as $$ select c from comments c where c.post_id = p $$;
create function out_row(p bigint, out r comments) returns setof comments language sql stable as $$ select c from comments c where c.post_id = p $$;

-- A derived table's or a WITH query's row reaches a function of a table's row type by position, and
-- the body reads its fields by name.
create function only_archived(t posts) returns boolean language sql stable as $$ select not t.archived $$;
