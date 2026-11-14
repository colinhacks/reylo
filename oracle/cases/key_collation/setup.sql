-- A comparison of a column under the default collation with a value that carries a
-- nondeterministic one runs under the value's, so one value matches several keys: `u.code` is
-- unique bytewise, and 'a' equals both 'a' and 'A' under `ci`. A key under `ci` itself holds to
-- that equality, and a value with no such collation leaves the column's own.
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
create domain ci_text as text collate ci;
create type ci_row as (name text collate ci);
create table ci_tab (id int primary key, c text collate ci not null, d text, r ci_row);
create table ci_dom (id int primary key, c ci_text not null);
create table u (id int primary key, code text unique);
create table ci_u (id int primary key, code text collate ci unique);
create table ci_code (id int primary key, code text collate ci not null);
create view ci_view as select id, c, d from ci_tab;
create function ci_value() returns ci_text language sql stable as $$ select 'a'::ci_text $$;
create function plain_value() returns text language sql stable as $$ select 'a'::text $$;
insert into ci_tab values (1, 'a', 'a', row('a')), (2, 'b', 'b', row('b'));
insert into ci_dom values (1, 'a');
insert into u values (1, 'a'), (2, 'A');
insert into ci_u values (1, 'a');
insert into ci_code values (1, 'a');
