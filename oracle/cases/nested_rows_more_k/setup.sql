-- A bare name in an item that is also a FROM item's name, where the column it reads first is no catalog table's: a derived table's, a WITH query's, a function's, a VALUES list's or an alias list's, several queries out, quoted, and read as c.*, (c).f or c::text.
create type addr as (street text, zip int);
create table users (id bigint primary key, name text not null);
create table aa (id int primary key, a addr);
create function addr_c() returns table (c addr) language sql stable as $$ select row('s', 1)::addr union all select null $$;
insert into users values (1, 'ada'), (2, 'bob');
insert into aa values (1, row('s', 1)), (2, null);
