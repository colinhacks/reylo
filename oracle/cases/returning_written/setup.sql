-- A write's RETURNING reads the row as the write left it: a column the statement wrote a non-null
-- value to holds that value, unless something changes it on its way to the row. A BEFORE trigger
-- (on the table, or on the partition a row lands in), a rule, a view's INSTEAD OF trigger, a
-- foreign table's wrapper can each give NULL in its place.
create table t (id int primary key, note text, at timestamptz, n int8, tags text[], label text);
insert into t values (1, null, null, null, null, null), (2, 'b', now(), 2, '{x}', 'l');
create table src (id int primary key, note text);
insert into src values (1, null), (3, 'c');
create function null_note() returns trigger language plpgsql as $$ begin new.note := null; return new; end $$;
create table trig (id int primary key, note text);
insert into trig values (1, 'a');
create trigger trig_null before insert or update on trig for each row execute function null_note();
create table parts (id int, note text) partition by list (id);
create table parts_1 partition of parts for values in (1);
create table parts_2 partition of parts for values in (2);
insert into parts values (1, 'a');
create trigger parts_null before insert on parts_2 for each row execute function null_note();
create table ruled_log (id int, note text);
create table ruled (id int primary key, note text);
create rule ruled_insert as on insert to ruled do instead insert into ruled_log values (new.id, null) returning ruled_log.*;
create view tv as select id, note from t;
create trigger tv_insert instead of insert on tv for each row execute function null_note();
create type mood as enum ('calm', 'sharp');
create table moods (id int primary key, m mood);
create domain short as text check (length(value) < 5);
create table dom (id int primary key, d short);
create table defaults (id int primary key, note text default null, stamp timestamptz default now());
create type pair as (a int, b text);
create table w (a int, b text);
insert into w values (90006, null);
create table gen (id int primary key, a int, g int generated always as (a) stored);
insert into gen values (1, null);
create extension file_fdw;
create server fs foreign data wrapper file_fdw;
create foreign table ft (id int, note text) server fs options (program 'true');
