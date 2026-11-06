-- `f.*` as a function's argument is the whole row of the FROM item, and a function that returns
-- one value of no row type gives that value for it: a string or a number, never an object. A
-- function with several columns, one of a row type, one of a domain over a row type, and one read
-- WITH ORDINALITY or beside another in ROWS FROM gives a row.
create type one_field as (x int);
create domain one_field_domain as one_field;
create table posts (id bigint primary key, title text not null);
create table comments (id bigint primary key, post_id bigint not null, body text not null);
insert into posts values (1, 't1');
insert into comments values (1, 1, 'b1'), (2, 1, 'b2');
create function bodies_for(p bigint) returns table (body text) language sql stable as $$ select body from comments where post_id = p $$;
create function many_bodies(p bigint) returns setof text language sql stable as $$ select body from comments where post_id = p $$;
create function single_out(x bigint, out a bigint) language sql stable as $$ select x $$;
create function two_columns(p bigint) returns table (id bigint, body text) language sql stable as $$ select id, body from comments where post_id = p $$;
create function whole_comments(p bigint) returns setof comments language sql stable as $$ select * from comments where post_id = p $$;
create function one_field_rows() returns setof one_field language sql stable as $$ select 1 $$;
create function one_field_domain_rows() returns setof one_field_domain language sql stable as $$ select row(1)::one_field_domain $$;
create function any_record() returns setof record language sql stable as $$ select 1, 'x'::text $$;
