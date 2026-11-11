-- The fixture every Postgres row reads: the local server's driver_matrix schema, the Neon
-- proxy's container, and each PGlite instance. Re-running it resets the three tables.
create schema if not exists driver_matrix;
drop table if exists driver_matrix.people, driver_matrix.scratch, driver_matrix.uniq, driver_matrix.typed;
drop type if exists driver_matrix.mood, driver_matrix.pair;
create table driver_matrix.people (id int primary key, name text not null, active boolean not null, score int);
insert into driver_matrix.people values (1, 'ada', true, 10), (2, 'bob', false, null), (3, 'cy', true, 30);
create table driver_matrix.scratch (id serial primary key, tag text not null, n int);
create table driver_matrix.uniq (k text primary key);
insert into driver_matrix.uniq values ('taken');
-- One value of each type whose JavaScript representation drivers disagree on (S10), at values every
-- driver reads exactly (Bun.sql keeps a timestamp's milliseconds and throws on a NULL in an int4[]).
create type driver_matrix.mood as enum ('ok', 'sad');
create type driver_matrix.pair as (a int, b text);
create table driver_matrix.typed (id int primary key, big int8, amount numeric, day date, at timestamp(3), stamp timestamptz, doc json, docb jsonb, raw bytea, flag bool, f8 float8, tags text[], ints int4[], moods driver_matrix.mood[], span interval, uid uuid, pr driver_matrix.pair, pt point, bxs box[], seats int4range, many int8multirange);
insert into driver_matrix.typed values (1, 9007199254740993, 12345678901234567890.123456789, '2026-10-20', '2026-10-20 10:00:00.123', '2026-10-20 10:00:00.123+02', '{"a": [1, 2]}', '{"b": true}', '\x0102ff', true, 2.5, '{a,NULL,"NULL","b,c"}', '{1,2,3}', '{ok,sad}', '1 day 2 hours', 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11', row(1, 'x,y'), '(1.5,-2)', '{(3,4),(1,2);(1,1),(0,0)}', '[1,5)', '{[1,3),[5,7)}');
