-- A SQL function's body, read as the query it is: what the function declares stays the column's
-- name and type, and its body narrows nullability, cardinality and JSON shapes where the call
-- returns what the body returns. Each trap below is a way a call differs from its body pasted in.
create table posts (id bigint primary key, title text not null, author_id bigint not null, archived boolean not null default false, done boolean not null default false, subtitle text);
create table comments (id bigint primary key, post_id bigint not null references posts, body text not null, note text);
create table pairs (a int not null, b int);
create view visible_comments as select id, post_id, body, note from comments where note is null;

-- The shapes a call returns.
create function comments_for(p bigint) returns table (id bigint, body text) language sql stable as $$ select id, body from comments where post_id = p $$;
create function one_comment(x bigint) returns table (id bigint, body text) language sql stable as $$ select id, body from comments where id = x $$;
create function comments_of_post(p bigint) returns setof comments language sql stable as $$ select * from comments where post_id = p $$;
create function out_set(p bigint, out a bigint, out b text) returns setof record language sql stable as $$ select id, note from comments where post_id = p $$;
create function no_arguments() returns table (id bigint) language sql stable as $$ select id from comments $$;
create function many_bodies(p bigint) returns setof text language sql stable as $$ select body from comments where post_id = p $$;
create function union_body(p bigint) returns table (id bigint) language sql stable as $$ select id from comments where post_id = p union all select post_id from comments $$;
create function with_body(p bigint) returns table (id bigint, body text) language sql stable as $$ with c as (select id, body from comments where post_id = p) select id, body from c $$;
create function from_view(p bigint) returns table (id bigint, body text) language sql stable as $$ select id, body from visible_comments where post_id = p $$;
create function reads_a_function(p bigint) returns table (id bigint, body text) language sql stable as $$ select id, body from comments_for(p) where id > 0 $$;
create function the_default_volatility(p bigint) returns table (id bigint, body text) language sql as $$ select id, body from comments where post_id = p $$;
create function volatile_key(x bigint) returns table (id bigint, body text) language sql volatile as $$ select id, body from comments where id = x $$;
create function definer(x bigint) returns table (id bigint, body text) language sql stable security definer as $$ select id, body from comments where id = x $$;
create schema app;
create function app.scoped(x bigint) returns table (id bigint, body text) language sql stable as $$ select id, body from comments where id = x $$;

-- A body stored parsed.
create function atomic_comments(p bigint) returns table (id bigint, body text) language sql stable begin atomic select id, body from comments where post_id = p; end;
create function atomic_key(x bigint) returns table (id bigint, body text) language sql stable begin atomic select c.id, c.body from comments c where c.id = x; end;
create function atomic_pinned(x bigint) returns table (id bigint, body text) language sql stable set search_path = public begin atomic select id, body from comments where id = x; end;
create function returned(a int, b int) returns int language sql stable return a + b;
create function atomic_two(p bigint) returns table (id bigint) language sql begin atomic select id from comments; select post_id from comments where id = p; end;

-- A call that returns one row, whatever its body returns: the first row it returns, or NULLs.
create function one_row(x bigint) returns comments language sql stable as $$ select * from comments where id = x $$;
create function first_of_post(p bigint) returns comments language sql stable as $$ select * from comments where post_id = p $$;
create function out_row(x bigint, out a bigint, out b text) language sql stable as $$ select id, body from comments where id = x $$;
create function stats(p bigint, out n bigint, out m bigint) language sql stable as $$ select count(*), max(id) from comments where post_id = p $$;
create function body_of(x bigint) returns text language sql stable as $$ select body from comments where id = x $$;
create function count_of(p bigint) returns bigint language sql stable as $$ select count(*) from comments where post_id = p $$;
create function joined(a text, b text) returns text language sql immutable as $$ select a || ' ' || b $$;
create function no_row() returns bigint language sql stable as $$ select 1::bigint where false $$;

-- Arguments.
create function shadowed(id bigint) returns table (found bigint) language sql stable as $$ select c.id from comments c where id = id $$;
create function shadow_inner(id bigint) returns table (found bigint) language sql stable as $$ select c.id from comments c where c.post_id in (select p.id from posts p where id = id) $$;
create function qualified(id bigint) returns table (found bigint) language sql stable as $$ select c.id from comments c where c.id = qualified.id $$;
create function by_number(bigint) returns table (id bigint) language sql stable as $$ select id from comments where id = $1 $$;
create function with_default(x bigint default 1) returns table (id bigint) language sql stable as $$ select id from comments where id = x $$;
create function second_default(x bigint, y bigint default null) returns table (id bigint, y bigint) language sql stable as $$ select id, y from comments where id = x $$;
create function variadic_ids(variadic ids bigint[]) returns table (id bigint) language sql stable as $$ select id from comments where id = any (ids) $$;
create function poly(x anyelement) returns setof anyelement language sql stable as $$ select x $$;
create function compatible(x anycompatible, y anycompatible) returns table (a anycompatible) language sql stable as $$ select coalesce(x, y) $$;
create function echo(x bigint) returns table (x2 bigint) language sql stable as $$ select x $$;
create function strict_scalar(p bigint) returns text language sql stable strict as $$ select 'x' $$;
create function strict_set(p bigint) returns table (a text) language sql stable strict as $$ select 'x' $$;
create function strict_out(p bigint, out a text) language sql stable strict as $$ select 'x' $$;
create function alias_named_p(x bigint) returns table (id bigint) language sql stable as $$ select p.id from comments p where p.post_id = x $$;
create function overloaded(x bigint) returns table (id bigint) language sql stable as $$ select id from comments where id = x $$;
create function overloaded(x text) returns table (id bigint) language sql stable as $$ select 1::bigint where false $$;
create function notes_for(p bigint) returns table (note text) language sql stable as $$ select note from comments where post_id = p $$;

-- Result columns go by position, and a body's last value is coerced to what the function declares.
create function swapped() returns table (a int, b int) language sql stable as $$ select b, a from pairs $$;
create function coerced() returns table (a bigint, b text) language sql stable as $$ select a, b::varchar from pairs $$;
create function whole_row() returns setof comments language sql stable as $$ select c from comments c $$;
create function jsonb_from_json(p bigint) returns jsonb language sql stable as $$ select '[]'::jsonb $$;

-- Bodies not read.
create function two_statements(p bigint) returns table (x bigint) language sql as $$ select id from comments; select post_id from comments where id = p $$;
create function in_plpgsql(x bigint) returns table (id bigint, body text) language plpgsql stable as $$ begin return query select c.id, c.body from comments c where c.id = x; end $$;
create function own_path(x bigint) returns table (id bigint, body text) language sql stable set search_path = public as $$ select id, body from comments where id = x $$;
create function recursive_one(n int) returns table (n2 int) language sql stable as $$ select n where n <= 0 union all select n2 from recursive_one(n - 1) where n > 0 $$;
create function cycle_a(n int) returns table (x int) language sql stable as $$ select n where n <= 0 $$;
create function cycle_b(n int) returns table (x int) language sql stable as $$ select x from cycle_a(n - 1) where n > 0 $$;
create or replace function cycle_a(n int) returns table (x int) language sql stable as $$ select n where n <= 0 union all select x from cycle_b(n) where n > 0 $$;
create function writes(x bigint) returns table (id bigint) language sql as $$ update comments set note = note where id = x returning id $$;
set check_function_bodies = off;
create function no_such_table() returns table (id bigint) language sql stable as $$ select id from nowhere $$;
create function no_such_syntax() returns table (id bigint) language sql stable as $$ selec 1 $$;
reset check_function_bodies;

-- Scalars in expressions, on a whole row.
create function is_open(t posts) returns boolean language sql stable as $$ select not t.archived and not t.done $$;
create function is_open_numbered(posts) returns boolean language sql stable as $$ select not ($1).archived and not ($1).done $$;
create function has_subtitle(t posts) returns boolean language sql stable as $$ select t.subtitle is not null $$;
create function strict_open(t posts) returns boolean language sql stable strict as $$ select not t.archived $$;

create table holder (id bigint primary key, t posts, archived boolean not null default false, done boolean not null default false);

-- JSON through a body.
create function comments_json(p bigint) returns jsonb language sql stable as $$ select coalesce(jsonb_agg(c), '[]'::jsonb) from (select id, body from comments where post_id = p) c $$;
create function comment_object(x bigint) returns jsonb language sql stable as $$ select jsonb_build_object('id', id, 'body', body) from comments where id = x $$;
create function table_of_json(p bigint) returns table (id bigint, doc jsonb) language sql stable as $$ select id, jsonb_build_object('body', body) from comments where post_id = p $$;

-- An aggregate of an argument belongs to the caller's query, written out: the body must stay the body.
create function own_sum(a int) returns bigint language sql stable as $$ select sum(a) $$;
create function own_count(a int) returns bigint language sql stable as $$ select count(a) $$;
create function counted_table(a int) returns table (n bigint) language sql stable as $$ select count(a) $$;
create function ranked(a int) returns bigint language sql stable as $$ select row_number() over () $$;
create function series_scalar(a int) returns int language sql stable as $$ select generate_series(1, a) $$;
create function varchar_column() returns table (b text) language sql stable as $$ select title::varchar from posts $$;
create function chain_8(p bigint) returns table (id bigint) language sql stable as $$ select id from comments where post_id = p $$;
create function chain_7(p bigint) returns table (id bigint) language sql stable as $$ select id from chain_8(p) $$;
create function chain_6(p bigint) returns table (id bigint) language sql stable as $$ select id from chain_7(p) $$;
create function chain_5(p bigint) returns table (id bigint) language sql stable as $$ select id from chain_6(p) $$;
create function chain_4(p bigint) returns table (id bigint) language sql stable as $$ select id from chain_5(p) $$;
create function chain_3(p bigint) returns table (id bigint) language sql stable as $$ select id from chain_4(p) $$;
create function chain_2(p bigint) returns table (id bigint) language sql stable as $$ select id from chain_3(p) $$;
create function chain_1(p bigint) returns table (id bigint) language sql stable as $$ select id from chain_2(p) $$;

-- More of what a call is and its body written out is not.
create function top_comment(p bigint) returns setof comments language sql stable as $$ select * from comments where post_id = p order by id limit 1 $$;
create function bumped(inout a int, out b text) language sql stable as $$ select a + 1, 'x'::text $$;
create domain positive as int check (value > 0);
create function by_domain(x positive) returns table (a int) language sql stable as $$ select a from pairs where a = x $$;
create function to_domain(x int) returns positive language sql stable as $$ select 1 $$;
create table thinned (a int not null, drop_me int, b text not null);
alter table thinned drop column drop_me;
create function thinned_rows() returns setof thinned language sql stable as $$ select * from thinned $$;
create function descendants(root bigint) returns table (id bigint) language sql stable as $$ with recursive d as (select c.id from comments c where c.post_id = root union all select c.id from comments c join d on c.post_id = d.id) select id from d $$;
create function "Quoted Name"("Arg One" bigint) returns table ("Out One" bigint) language sql stable as $$ select id from comments where post_id = "Arg One" $$;
create function slice(a int[]) returns int[] language sql stable as $$ select a[1:2] $$;
create function slice_to(a int[], n int) returns int[] language sql stable as $$ select a[1:n] $$;
create function count_as_given(n bigint) returns bigint language sql stable as $$ select n $$;
create type cast_named as (a int);
create function cast_named(x bigint) returns table (a int) language sql stable as $$ select 1 $$;
create function left_joined(p bigint) returns table (id bigint, note text) language sql stable as $$ select c.id, o.note from comments c left join comments o on o.id = c.id + 1 where c.post_id = p $$;
create function grouped(p bigint) returns table (post_id bigint, n bigint) language sql stable as $$ select post_id, count(*) from comments where post_id = p group by post_id $$;

-- A search path of its own: a body that names its schemas means the same under any path, and another setting says nothing of names.
create function own_empty_path(x bigint) returns table (id bigint, body text) language sql stable set search_path = '' as $$ select id, body from public.comments where id = x $$;
create function own_setting(x bigint) returns table (id bigint, body text) language sql stable set work_mem = '64kB' as $$ select id, body from comments where id = x $$;
create function own_path_unqualified_call(x bigint) returns table (id bigint) language sql stable set search_path = public as $$ select id from comments_for(x) $$;

-- A row-dependent value in a body must not look constant to the proof of the statement around it.
create function same_id(x bigint) returns bigint language sql stable as $$ select x $$;
create function lookup_id(x bigint) returns bigint language sql stable as $$ select c.id from comments c where c.id = x $$;
create function by_key_or_null(x bigint) returns table (id bigint) language sql stable as $$ select id from comments where x is null or id = x $$;

-- A body that shows one value where a call's changes from row to row, under a subquery.
create function pick(x bigint) returns bigint language sql stable as $$ select x from (select 1) s $$;
create function lit(x bigint) returns table (v bigint) language sql stable as $$ select x $$;
-- A strict function reached through a body, where the argument may be null.
create function g_strict(x text) returns text language sql stable strict as $$ select 'x' $$;
create function f_through_strict(x text) returns text language sql stable as $$ select g_strict(x) $$;
create function g_strict_set(x text) returns table (v text) language sql stable strict as $$ select 'x' $$;
create function f_through_strict_set(x text) returns table (v text) language sql stable as $$ select v from g_strict_set(x) $$;
