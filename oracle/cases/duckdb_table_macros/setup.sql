-- A table macro's body stands in its place when DuckDB binds the statement, so what the body says of
-- its columns and its rows holds of the call. A macro declares no result type, so the prepared
-- statement says the types, and the body narrows them.
create table posts (id bigint primary key, title varchar not null, subtitle varchar);
create table comments (id bigint primary key, post_id bigint not null, body varchar not null, note varchar);
create table codes (code varchar primary key, n int not null);
create view open_comments as select id, body from comments where note is null;
insert into posts values (1, 't', null), (2, 'u', 's');
insert into comments values (1, 1, 'a', null), (2, 1, 'b', 'n'), (3, 2, 'c', null);
insert into codes values ('1', 1), ('01', 2);

create macro comments_for(p) as table select id, body from comments where post_id = p;
create macro one_comment(x) as table select id, body from comments where id = x;
create macro all_of(p) as table select * from comments where post_id = p;
create macro with_note(p) as table select id, note from comments where post_id = p;
create macro counted(p) as table select count(*) as n, max(id) as m from comments where post_id = p;
create macro grouped(p) as table select post_id, count(*) as n from comments where post_id = p group by post_id;
create macro limited(p) as table select id, body from comments where post_id = p order by id limit 1;
create macro defaulted(p := 1) as table select id from comments where post_id = p;
create macro through_macro(p) as table select id, body from comments_for(p);
create macro from_first(p) as table from comments select id, body where post_id = p;
create macro with_query(p) as table with c as (select id, body from comments where post_id = p) select id, body from c;
create macro from_view() as table select id, body from open_comments;
create macro unioned(p) as table select id from comments where post_id = p union all select post_id from comments;
create macro left_joined(p) as table select c.id, o.note from comments c left join comments o on o.id = c.id + 1 where c.post_id = p;

-- A column beats a parameter of the same name.
create macro shadowed(id) as table select c.id as found from comments c where id = id;

-- An argument passes through as it is.
create macro echoed(x) as table select x as v;
create macro echoed_row(x) as table select x as v, id from comments;

-- A row maker in a body.
create macro unnested(l) as table select unnest(l) as v;
create macro one_per_element(p) as table select id, unnest([1, 2]) as v from comments where id = p;

-- A key compared with a value of another type: DuckDB converts one side, and two keys may become one value.
create macro by_code(c) as table select code, n from codes where code = c;
create macro through_code(x) as table select code, n from by_code(x);
create macro by_code_as_number(c) as table select code, n from codes where code = c::int8;
create table amounts (amount decimal(38,2) primary key, n int not null);
insert into amounts values (9007199254740992.00, 1), (9007199254740992.01, 2);
create table big_ids (id bigint primary key, n int not null);
insert into big_ids values (9007199254740992, 1), (9007199254740993, 2);
create type mood as enum ('1', '01');
create table moods (m mood primary key, n int not null);
insert into moods values ('1', 1), ('01', 2);

-- A key under a collation holds values that compare equal.
create table ci_codes (code varchar collate nocase primary key, n int not null);
insert into ci_codes values ('a', 1), ('A', 2);

-- Several macros of one name, and one that shares a built-in's: neither is read.
create macro overloaded(a) as table select id from comments where id = a, (a, b) as table select id from comments where id = a and post_id = b;
create macro pick(v) as v;

-- A macro reading another, in a cycle of its own making is refused when it is made.
create macro level_6(p) as table select id from comments where post_id = p;
create macro level_5(p) as table select id from level_6(p);
create macro level_4(p) as table select id from level_5(p);
create macro level_3(p) as table select id from level_4(p);
create macro level_2(p) as table select id from level_3(p);
create macro level_1(p) as table select id from level_2(p);
create macro level_0(p) as table select id from level_1(p);
