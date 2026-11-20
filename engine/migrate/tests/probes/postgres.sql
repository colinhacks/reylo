-- One probe per property Postgres's DDL can set: two schemas that differ in that property alone,
-- each planned both ways by tests/second_opinion.rs. Roles gap_r1, gap_r2 and gap_owner exist.

-- ===== aggregates (CREATE/ALTER AGGREGATE)
-- probe: aggregate-added
-- base:
create function add2(int, int) returns int language sql immutable as 'select $1 + $2';
-- v2:
create aggregate mysum(int) (sfunc = add2, stype = int, initcond = '0');

-- probe: aggregate-initcond
-- base:
create function add2(int, int) returns int language sql immutable as 'select $1 + $2';
-- v1:
create aggregate mysum(int) (sfunc = add2, stype = int, initcond = '0');
-- v2:
create aggregate mysum(int) (sfunc = add2, stype = int, initcond = '1');

-- probe: aggregate-parallel
-- base:
create function add2(int, int) returns int language sql immutable parallel safe as 'select $1 + $2';
-- v1:
create aggregate mysum(int) (sfunc = add2, stype = int);
-- v2:
create aggregate mysum(int) (sfunc = add2, stype = int, parallel = safe);

-- probe: aggregate-owner
-- base:
create function add2(int, int) returns int language sql immutable as 'select $1 + $2';
create aggregate mysum(int) (sfunc = add2, stype = int);
-- v2:
alter aggregate mysum(int) owner to gap_owner;

-- probe: aggregate-comment
-- base:
create function add2(int, int) returns int language sql immutable as 'select $1 + $2';
create aggregate mysum(int) (sfunc = add2, stype = int);
-- v2:
comment on aggregate mysum(int) is 'sums';

-- probe: aggregate-grant
-- base:
create function add2(int, int) returns int language sql immutable as 'select $1 + $2';
create aggregate mysum(int) (sfunc = add2, stype = int);
-- v2:
revoke execute on function mysum(int) from public;
grant execute on function mysum(int) to gap_r1;

-- ===== collations (CREATE/ALTER COLLATION)
-- probe: collation-added
-- v2:
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);

-- probe: collation-locale
-- v1:
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
-- v2:
create collation ci (provider = icu, locale = 'und-u-ks-level1', deterministic = false);

-- probe: collation-deterministic
-- v1:
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = true);
-- v2:
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);

-- probe: collation-rules
-- v1:
create collation r (provider = icu, locale = 'und');
-- v2:
create collation r (provider = icu, locale = 'und', rules = '&a < g');

-- probe: collation-libc
-- v1:
create collation c2 (locale = 'C');
-- v2:
create collation c2 (locale = 'POSIX');

-- probe: collation-owner
-- base:
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
-- v2:
alter collation ci owner to gap_owner;

-- probe: collation-comment
-- base:
create collation ci (provider = icu, locale = 'und-u-ks-level2', deterministic = false);
-- v2:
comment on collation ci is 'case-insensitive';

-- ===== conversions (CREATE CONVERSION)
-- probe: conversion-added
-- v2:
create conversion myconv for 'LATIN1' to 'UTF8' from iso8859_1_to_utf8;

-- probe: conversion-default
-- v1:
create conversion myconv for 'LATIN1' to 'UTF8' from iso8859_1_to_utf8;
-- v2:
create default conversion myconv for 'LATIN1' to 'UTF8' from iso8859_1_to_utf8;

-- probe: conversion-comment
-- base:
create conversion myconv for 'LATIN1' to 'UTF8' from iso8859_1_to_utf8;
-- v2:
comment on conversion myconv is 'latin';

-- probe: conversion-owner
-- base:
create conversion myconv for 'LATIN1' to 'UTF8' from iso8859_1_to_utf8;
-- v2:
alter conversion myconv owner to gap_owner;

-- ===== default privileges (ALTER DEFAULT PRIVILEGES)
-- probe: default-privileges-tables
-- v2:
alter default privileges grant select on tables to gap_r1;

-- probe: default-privileges-in-schema
-- base:
create schema s;
-- v2:
alter default privileges in schema s grant insert on tables to gap_r1;

-- probe: default-privileges-for-role
-- v2:
alter default privileges for role gap_owner grant select on tables to gap_r1;

-- probe: default-privileges-functions-revoked
-- v2:
alter default privileges revoke execute on functions from public;

-- probe: default-privileges-sequences
-- v2:
alter default privileges grant usage on sequences to gap_r1;

-- probe: default-privileges-types
-- v2:
alter default privileges revoke usage on types from public;

-- probe: default-privileges-schemas
-- v2:
alter default privileges grant usage on schemas to gap_r1;

-- probe: default-privileges-grant-option
-- v1:
alter default privileges grant select on tables to gap_r1;
-- v2:
alter default privileges grant select on tables to gap_r1 with grant option;

-- ===== domains (CREATE/ALTER DOMAIN)
-- probe: domain-added
-- v2:
create domain pos as int check (value > 0);

-- probe: domain-check
-- v1:
create domain pos as int check (value > 0);
-- v2:
create domain pos as int check (value > 1);

-- probe: domain-not-null
-- v1:
create domain d as int;
-- v2:
create domain d as int not null;

-- probe: domain-default
-- v1:
create domain d as int;
-- v2:
create domain d as int default 7;

-- probe: domain-collation
-- v1:
create domain d as text;
-- v2:
create domain d as text collate "C";

-- probe: domain-constraint-not-valid
-- base:
create domain d as int;
-- v1:
alter domain d add constraint pos check (value > 0);
-- v2:
alter domain d add constraint pos check (value > 0) not valid;

-- probe: domain-constraint-name
-- v1:
create domain d as int constraint c1 check (value > 0);
-- v2:
create domain d as int constraint c2 check (value > 0);

-- probe: domain-owner
-- base:
create domain d as int;
-- v2:
alter domain d owner to gap_owner;

-- probe: domain-comment
-- base:
create domain d as int;
-- v2:
comment on domain d is 'a d';

-- probe: domain-constraint-comment
-- base:
create domain d as int constraint pos check (value > 0);
-- v2:
comment on constraint pos on domain d is 'positive';

-- probe: domain-grant
-- base:
create domain d as int;
-- v2:
revoke usage on domain d from public;

-- probe: domain-array-base
-- v1:
create domain d as int[];
-- v2:
create domain d as bigint[];

-- ===== event triggers (CREATE/ALTER EVENT TRIGGER)
-- probe: event-trigger-added
-- base:
create function et() returns event_trigger language plpgsql as $$ begin end $$;
-- v2:
create event trigger e on ddl_command_end execute function et();

-- probe: event-trigger-tags
-- base:
create function et() returns event_trigger language plpgsql as $$ begin end $$;
-- v1:
create event trigger e on ddl_command_end execute function et();
-- v2:
create event trigger e on ddl_command_end when tag in ('CREATE TABLE') execute function et();

-- probe: event-trigger-event
-- base:
create function et() returns event_trigger language plpgsql as $$ begin end $$;
-- v1:
create event trigger e on ddl_command_end execute function et();
-- v2:
create event trigger e on ddl_command_start execute function et();

-- probe: event-trigger-disabled
-- base:
create function et() returns event_trigger language plpgsql as $$ begin end $$;
create event trigger e on ddl_command_end execute function et();
-- v2:
alter event trigger e disable;

-- probe: event-trigger-enable-always
-- base:
create function et() returns event_trigger language plpgsql as $$ begin end $$;
create event trigger e on ddl_command_end execute function et();
-- v2:
alter event trigger e enable always;

-- probe: event-trigger-owner
-- base:
create function et() returns event_trigger language plpgsql as $$ begin end $$;
create event trigger e on ddl_command_end execute function et();
-- v2:
alter event trigger e owner to gap_super;

-- probe: event-trigger-comment
-- base:
create function et() returns event_trigger language plpgsql as $$ begin end $$;
create event trigger e on ddl_command_end execute function et();
-- v2:
comment on event trigger e is 'audit';

-- probe: event-trigger-login
-- base:
create function et() returns event_trigger language plpgsql as $$ begin end $$;
-- v2:
create event trigger e on login execute function et();

-- ===== extensions (CREATE/ALTER EXTENSION)
-- probe: extension-added
-- v2:
create extension citext;

-- probe: extension-schema
-- base:
create schema ext;
-- v1:
create extension citext;
-- v2:
create extension citext schema ext;

-- probe: extension-comment
-- base:
create extension citext;
-- v2:
comment on extension citext is 'mine';

-- probe: extension-member-grant
-- base:
create extension pgcrypto;
-- v2:
revoke execute on function digest(text, text) from public;
grant execute on function digest(text, text) to gap_r1;

-- probe: extension-member-comment
-- base:
create extension pgcrypto;
-- v2:
comment on function digest(text, text) is 'hash';

-- probe: extension-member-dropped
-- base:
create extension pgcrypto;
-- v2:
alter extension pgcrypto drop function gen_salt(text);

-- probe: extension-table-grant
-- base:
create extension postgres_fdw;
-- v2:
grant usage on foreign data wrapper postgres_fdw to gap_r1;

-- ===== foreign data (FDW, SERVER, USER MAPPING, FOREIGN TABLE)
-- probe: fdw-added
-- v2:
create foreign data wrapper w;

-- probe: fdw-options
-- v1:
create foreign data wrapper w options (a '1');
-- v2:
create foreign data wrapper w options (a '2');

-- probe: fdw-handler
-- base:
create extension postgres_fdw;
-- v1:
create foreign data wrapper w;
-- v2:
create foreign data wrapper w handler postgres_fdw_handler;

-- probe: fdw-owner
-- base:
create foreign data wrapper w;
-- v2:
alter foreign data wrapper w owner to gap_super;

-- probe: fdw-grant
-- base:
create foreign data wrapper w;
-- v2:
grant usage on foreign data wrapper w to gap_r1;

-- probe: fdw-comment
-- base:
create foreign data wrapper w;
-- v2:
comment on foreign data wrapper w is 'w';

-- probe: server-added
-- base:
create extension postgres_fdw;
-- v2:
create server s foreign data wrapper postgres_fdw options (host 'h', dbname 'd');

-- probe: server-options
-- base:
create extension postgres_fdw;
-- v1:
create server s foreign data wrapper postgres_fdw options (host 'h1');
-- v2:
create server s foreign data wrapper postgres_fdw options (host 'h2');

-- probe: server-type-version
-- base:
create extension postgres_fdw;
-- v1:
create server s foreign data wrapper postgres_fdw;
-- v2:
create server s type 'x' version '2' foreign data wrapper postgres_fdw;

-- probe: server-owner
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v2:
alter server s owner to gap_owner;

-- probe: server-grant
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v2:
grant usage on foreign server s to gap_r1;

-- probe: server-comment
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v2:
comment on server s is 'remote';

-- probe: user-mapping-added
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v2:
create user mapping for gap_r1 server s options (user 'u');

-- probe: user-mapping-options
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v1:
create user mapping for gap_r1 server s options (user 'u1');
-- v2:
create user mapping for gap_r1 server s options (user 'u2');

-- probe: user-mapping-public
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v2:
create user mapping for public server s;

-- probe: foreign-table-added
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v2:
create foreign table ft (a int) server s options (table_name 'x');

-- probe: foreign-table-options
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v1:
create foreign table ft (a int) server s options (table_name 'x');
-- v2:
create foreign table ft (a int) server s options (table_name 'y');

-- probe: foreign-table-column-options
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v1:
create foreign table ft (a int) server s;
-- v2:
create foreign table ft (a int options (column_name 'b')) server s;

-- probe: foreign-table-server
-- base:
create extension postgres_fdw;
create server s1 foreign data wrapper postgres_fdw;
create server s2 foreign data wrapper postgres_fdw;
-- v1:
create foreign table ft (a int) server s1;
-- v2:
create foreign table ft (a int) server s2;

-- probe: foreign-table-not-null
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v1:
create foreign table ft (a int) server s;
-- v2:
create foreign table ft (a int not null) server s;

-- probe: foreign-table-default
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v1:
create foreign table ft (a int) server s;
-- v2:
create foreign table ft (a int default 3) server s;

-- probe: foreign-table-check
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v1:
create foreign table ft (a int) server s;
-- v2:
create foreign table ft (a int check (a > 0)) server s;

-- probe: foreign-table-grant
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
create foreign table ft (a int) server s;
-- v2:
grant select on ft to gap_r1;

-- probe: foreign-table-comment
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
create foreign table ft (a int) server s;
-- v2:
comment on foreign table ft is 'remote';

-- probe: foreign-table-partition
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
create table p (id int) partition by range (id);
-- v2:
create foreign table p1 partition of p for values from (0) to (10) server s;

-- ===== functions and procedures (CREATE/ALTER FUNCTION, PROCEDURE, ROUTINE)
-- probe: function-added
-- v2:
create function f() returns int language sql as 'select 1';

-- probe: function-body
-- v1:
create function f() returns int language sql as 'select 1';
-- v2:
create function f() returns int language sql as 'select 2';

-- probe: function-volatility
-- v1:
create function f() returns int language sql as 'select 1';
-- v2:
create function f() returns int language sql immutable as 'select 1';

-- probe: function-strict
-- v1:
create function f(int) returns int language sql as 'select $1';
-- v2:
create function f(int) returns int language sql strict as 'select $1';

-- probe: function-security-definer
-- v1:
create function f() returns int language sql as 'select 1';
-- v2:
create function f() returns int language sql security definer as 'select 1';

-- probe: function-leakproof
-- v1:
create function f(int) returns bool language sql immutable as 'select $1 > 0';
-- v2:
create function f(int) returns bool language sql immutable leakproof as 'select $1 > 0';

-- probe: function-cost
-- v1:
create function f() returns int language sql as 'select 1';
-- v2:
create function f() returns int language sql cost 500 as 'select 1';

-- probe: function-rows
-- v1:
create function f() returns setof int language sql as 'select 1';
-- v2:
create function f() returns setof int language sql rows 5 as 'select 1';

-- probe: function-parallel
-- v1:
create function f() returns int language sql as 'select 1';
-- v2:
create function f() returns int language sql parallel safe as 'select 1';

-- probe: function-set-config
-- v1:
create function f() returns int language sql as 'select 1';
-- v2:
create function f() returns int language sql set search_path = public as 'select 1';

-- probe: function-arg-default
-- v1:
create function f(a int default 1) returns int language sql as 'select a';
-- v2:
create function f(a int default 2) returns int language sql as 'select a';

-- probe: function-arg-name
-- v1:
create function f(a int) returns int language sql as 'select $1';
-- v2:
create function f(b int) returns int language sql as 'select $1';

-- probe: function-returns-table
-- v1:
create function f() returns table (a int) language sql as 'select 1';
-- v2:
create function f() returns table (b int) language sql as 'select 1';

-- probe: function-sql-body
-- v1:
create function f() returns int language sql return 1;
-- v2:
create function f() returns int language sql return 2;

-- probe: function-owner
-- base:
create function f() returns int language sql as 'select 1';
-- v2:
alter function f() owner to gap_owner;

-- probe: function-comment
-- base:
create function f() returns int language sql as 'select 1';
-- v2:
comment on function f() is 'one';

-- probe: function-revoke-public
-- base:
create function f() returns int language sql as 'select 1';
-- v2:
revoke execute on function f() from public;

-- probe: function-grant
-- base:
create function f() returns int language sql as 'select 1';
-- v2:
grant execute on function f() to gap_r1 with grant option;

-- probe: function-depends-on-extension
-- base:
create extension citext;
create function f() returns int language sql as 'select 1';
-- v2:
alter function f() depends on extension citext;

-- probe: function-window
-- v1:
create function f() returns int language internal as 'window_row_number';
-- v2:
create function f() returns int language internal window as 'window_row_number';

-- probe: function-support
-- v1:
create function f(int) returns int language sql as 'select $1';
-- v2:
create function f(int) returns int language sql support generate_series_int4_support as 'select $1';

-- probe: procedure-added
-- v2:
create procedure p() language sql as 'select 1';

-- probe: procedure-body
-- v1:
create procedure p() language sql as 'select 1';
-- v2:
create procedure p() language sql as 'select 2';

-- probe: procedure-security
-- v1:
create procedure p() language sql as 'select 1';
-- v2:
create procedure p() language sql security definer as 'select 1';

-- probe: procedure-grant
-- base:
create procedure p() language sql as 'select 1';
-- v2:
revoke execute on procedure p() from public;

-- probe: procedure-comment
-- base:
create procedure p() language sql as 'select 1';
-- v2:
comment on procedure p() is 'p';

-- ===== indexes (CREATE/ALTER INDEX)
-- probe: index-added
-- base:
create table t (id int primary key, a int, b text);
-- v2:
create index i on t (a);

-- probe: index-unique
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (a);
-- v2:
create unique index i on t (a);

-- probe: index-partial
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (a);
-- v2:
create index i on t (a) where a > 0;

-- probe: index-expression
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (lower(b));
-- v2:
create index i on t (upper(b));

-- probe: index-include
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (a);
-- v2:
create index i on t (a) include (b);

-- probe: index-nulls-not-distinct
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create unique index i on t (a);
-- v2:
create unique index i on t (a) nulls not distinct;

-- probe: index-desc-nulls
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (a);
-- v2:
create index i on t (a desc nulls last);

-- probe: index-opclass
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (b);
-- v2:
create index i on t (b text_pattern_ops);

-- probe: index-collation
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (b);
-- v2:
create index i on t (b collate "C");

-- probe: index-method
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (a);
-- v2:
create index i on t using hash (a);

-- probe: index-brin-options
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t using brin (a);
-- v2:
create index i on t using brin (a) with (pages_per_range = 32);

-- probe: index-fillfactor
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (a);
-- v2:
create index i on t (a) with (fillfactor = 50);

-- probe: index-deduplicate
-- base:
create table t (id int primary key, a int, b text);
-- v1:
create index i on t (a);
-- v2:
create index i on t (a) with (deduplicate_items = off);

-- probe: index-column-statistics
-- base:
create table t (id int primary key, a int, b text);
create index i on t ((a + 1));
-- v2:
alter index i alter column 1 set statistics 500;

-- probe: index-cluster
-- base:
create table t (id int primary key, a int, b text);
create index i on t (a);
-- v2:
alter table t cluster on i;

-- probe: index-comment
-- base:
create table t (id int primary key, a int, b text);
create index i on t (a);
-- v2:
comment on index i is 'by a';

-- probe: index-depends-on-extension
-- base:
create extension citext;
create table t (id int primary key, a int, b text);
create index i on t (a);
-- v2:
alter index i depends on extension citext;

-- probe: index-gin-fastupdate
-- base:
create table t (id int primary key, a int[]);
-- v1:
create index i on t using gin (a);
-- v2:
create index i on t using gin (a) with (fastupdate = off);

-- probe: index-on-only-partitioned
-- base:
create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
-- v1:
create index i on p (a);
-- v2:
create index i on only p (a);

-- ===== access methods, languages, transforms, text search (read by name only or not read)
-- probe: access-method-added
-- v2:
create access method myheap type table handler heap_tableam_handler;

-- probe: table-access-method
-- base:
create access method myheap type table handler heap_tableam_handler;
-- v1:
create table t (id int);
-- v2:
create table t (id int) using myheap;

-- probe: language-grant
-- v2:
revoke usage on language plpgsql from public;

-- probe: language-grant-role
-- v2:
grant usage on language sql to gap_r1 with grant option;

-- probe: language-owner
-- v2:
alter language plpgsql owner to gap_owner;

-- probe: language-comment
-- v2:
comment on language plpgsql is 'pl';

-- probe: text-search-config-added
-- v2:
create text search configuration en (copy = english);

-- probe: text-search-config-mapping
-- base:
create text search configuration en (copy = english);
-- v2:
alter text search configuration en alter mapping for asciiword with simple;

-- probe: text-search-config-comment
-- base:
create text search configuration en (copy = english);
-- v2:
comment on text search configuration en is 'en';

-- probe: text-search-dictionary-options
-- v1:
create text search dictionary d (template = simple, stopwords = english);
-- v2:
create text search dictionary d (template = simple, stopwords = english, accept = false);

-- probe: text-search-template-added
-- v2:
create text search template tt (init = dsimple_init, lexize = dsimple_lexize);

-- ===== materialized views
-- probe: matview-added
-- v2:
create materialized view mv as select 1 as a;

-- probe: matview-query
-- v1:
create materialized view mv as select 1 as a;
-- v2:
create materialized view mv as select 2 as a;

-- probe: matview-with-no-data
-- v1:
create materialized view mv as select 1 as a;
-- v2:
create materialized view mv as select 1 as a with no data;

-- probe: matview-storage
-- v1:
create materialized view mv as select 1 as a;
-- v2:
create materialized view mv with (fillfactor = 70) as select 1 as a;

-- probe: matview-access-method
-- base:
create access method myheap type table handler heap_tableam_handler;
-- v1:
create materialized view mv as select 1 as a;
-- v2:
create materialized view mv using myheap as select 1 as a;

-- probe: matview-index
-- base:
create materialized view mv as select 1 as a;
-- v2:
create unique index mvi on mv (a);

-- probe: matview-cluster
-- base:
create materialized view mv as select 1 as a;
create index mvi on mv (a);
-- v2:
alter materialized view mv cluster on mvi;

-- probe: matview-column-statistics
-- base:
create materialized view mv as select 1 as a;
-- v2:
alter materialized view mv alter column a set statistics 500;

-- probe: matview-column-storage
-- base:
create materialized view mv as select 'x'::text as a;
-- v2:
alter materialized view mv alter column a set storage external;

-- probe: matview-column-compression
-- base:
create materialized view mv as select 'x'::text as a;
-- v2:
alter materialized view mv alter column a set compression lz4;

-- probe: matview-owner
-- base:
create materialized view mv as select 1 as a;
-- v2:
alter materialized view mv owner to gap_owner;

-- probe: matview-grant
-- base:
create materialized view mv as select 1 as a;
-- v2:
grant select on mv to gap_r1;

-- probe: matview-comment
-- base:
create materialized view mv as select 1 as a;
-- v2:
comment on materialized view mv is 'mv';

-- probe: matview-column-comment
-- base:
create materialized view mv as select 1 as a;
-- v2:
comment on column mv.a is 'a';

-- probe: matview-column-names
-- v1:
create materialized view mv (a) as select 1;
-- v2:
create materialized view mv (b) as select 1;

-- ===== operators, operator classes and families
-- probe: operator-added
-- base:
create function eqi(int, int) returns bool language sql immutable as 'select $1 = $2';
-- v2:
create operator === (leftarg = int, rightarg = int, function = eqi);

-- probe: operator-restrict
-- base:
create function eqi(int, int) returns bool language sql immutable as 'select $1 = $2';
-- v1:
create operator === (leftarg = int, rightarg = int, function = eqi);
-- v2:
create operator === (leftarg = int, rightarg = int, function = eqi, restrict = eqsel, join = eqjoinsel);

-- probe: operator-commutator
-- base:
create function eqi(int, int) returns bool language sql immutable as 'select $1 = $2';
-- v1:
create operator === (leftarg = int, rightarg = int, function = eqi);
-- v2:
create operator === (leftarg = int, rightarg = int, function = eqi, commutator = ===);

-- probe: operator-comment
-- base:
create function eqi(int, int) returns bool language sql immutable as 'select $1 = $2';
create operator === (leftarg = int, rightarg = int, function = eqi);
-- v2:
comment on operator === (int, int) is 'eq';

-- probe: operator-owner
-- base:
create function eqi(int, int) returns bool language sql immutable as 'select $1 = $2';
create operator === (leftarg = int, rightarg = int, function = eqi);
-- v2:
alter operator === (int, int) owner to gap_owner;

-- probe: operator-class-added
-- base:
create function cmpi(int, int) returns int language sql immutable as 'select case when $1 < $2 then -1 when $1 > $2 then 1 else 0 end';
-- v2:
create operator class myops for type int using btree as operator 1 <, operator 2 <=, operator 3 =, operator 4 >=, operator 5 >, function 1 cmpi(int, int);

-- probe: operator-class-function
-- base:
create domain pint as int;
create function cmpi(int, int) returns int language sql immutable as 'select case when $1 < $2 then -1 when $1 > $2 then 1 else 0 end';
-- v1:
create operator class myops for type int using hash as operator 1 =, function 1 hashint4(int);
-- v2:
create operator class myops for type int using hash as operator 1 =, function 1 hashint4(int), function 2 hashint4extended(int, bigint);

-- probe: operator-family-member
-- base:
create function cmpi(int, int) returns int language sql immutable as 'select case when $1 < $2 then -1 when $1 > $2 then 1 else 0 end';
create operator family fam using btree;
-- v2:
alter operator family fam using btree add function 1 (int, int) cmpi(int, int);

-- ===== policies and row-level security
-- probe: policy-added
-- base:
create table t (id int primary key, owner text);
-- v2:
create policy p on t using (owner = current_user);

-- probe: policy-using
-- base:
create table t (id int primary key, owner text);
-- v1:
create policy p on t using (owner = current_user);
-- v2:
create policy p on t using (owner = session_user);

-- probe: policy-with-check
-- base:
create table t (id int primary key, owner text);
-- v1:
create policy p on t for insert with check (true);
-- v2:
create policy p on t for insert with check (owner = current_user);

-- probe: policy-roles
-- base:
create table t (id int primary key, owner text);
-- v1:
create policy p on t using (true);
-- v2:
create policy p on t to gap_r1 using (true);

-- probe: policy-restrictive
-- base:
create table t (id int primary key, owner text);
-- v1:
create policy p on t using (true);
-- v2:
create policy p on t as restrictive using (true);

-- probe: policy-command
-- base:
create table t (id int primary key, owner text);
-- v1:
create policy p on t for select using (true);
-- v2:
create policy p on t for update using (true);

-- probe: policy-comment
-- base:
create table t (id int primary key, owner text);
create policy p on t using (true);
-- v2:
comment on policy p on t is 'mine';

-- probe: rls-enabled
-- base:
create table t (id int primary key);
-- v2:
alter table t enable row level security;

-- probe: rls-forced
-- base:
create table t (id int primary key);
alter table t enable row level security;
-- v2:
alter table t force row level security;

-- probe: rls-forced-only
-- base:
create table t (id int primary key);
-- v2:
alter table t force row level security;

-- ===== publications
-- probe: publication-added
-- base:
create table t (id int primary key, a int);
-- v2:
create publication pub for table t;

-- probe: publication-all-tables
-- base:
create table t (id int primary key, a int);
-- v1:
create publication pub;
-- v2:
create publication pub for all tables;

-- probe: publication-row-filter
-- base:
create table t (id int primary key, a int);
-- v1:
create publication pub for table t;
-- v2:
create publication pub for table t where (a > 0);

-- probe: publication-column-list
-- base:
create table t (id int primary key, a int);
-- v1:
create publication pub for table t;
-- v2:
create publication pub for table t (id);

-- probe: publication-schema
-- base:
create schema s;
-- v1:
create publication pub;
-- v2:
create publication pub for tables in schema s;

-- probe: publication-publish
-- base:
create table t (id int primary key, a int);
-- v1:
create publication pub for table t;
-- v2:
create publication pub for table t with (publish = 'insert');

-- probe: publication-via-root
-- base:
create table p (id int) partition by range (id);
-- v1:
create publication pub for table p;
-- v2:
create publication pub for table p with (publish_via_partition_root = true);

-- probe: publication-generated-columns
-- base:
create table t (id int primary key, a int generated always as (id * 2) stored);
-- v1:
create publication pub for table t;
-- v2:
create publication pub for table t with (publish_generated_columns = stored);

-- probe: publication-owner
-- base:
create publication pub;
-- v2:
alter publication pub owner to gap_owner;

-- probe: publication-comment
-- base:
create publication pub;
-- v2:
comment on publication pub is 'cdc';

-- ===== rules
-- probe: rule-added
-- base:
create table t (id int primary key, a int);
create table log (id int);
-- v2:
create rule r as on insert to t do also insert into log values (new.id);

-- probe: rule-changed
-- base:
create table t (id int primary key, a int);
create table log (id int);
-- v1:
create rule r as on insert to t do also insert into log values (new.id);
-- v2:
create rule r as on insert to t do also insert into log values (new.a);

-- probe: rule-disabled
-- base:
create table t (id int primary key, a int);
create table log (id int);
create rule r as on insert to t do also insert into log values (new.id);
-- v2:
alter table t disable rule r;

-- probe: rule-enable-replica
-- base:
create table t (id int primary key, a int);
create table log (id int);
create rule r as on insert to t do also insert into log values (new.id);
-- v2:
alter table t enable replica rule r;

-- probe: rule-comment
-- base:
create table t (id int primary key, a int);
create table log (id int);
create rule r as on insert to t do also insert into log values (new.id);
-- v2:
comment on rule r on t is 'log';

-- ===== schemas
-- probe: schema-added
-- v2:
create schema s;

-- probe: schema-owner
-- base:
create schema s;
-- v2:
alter schema s owner to gap_owner;

-- probe: schema-authorization
-- v1:
create schema s;
-- v2:
create schema s authorization gap_owner;

-- probe: schema-grant
-- base:
create schema s;
-- v2:
grant usage on schema s to gap_r1;

-- probe: schema-public-revoke
-- v2:
revoke create on schema public from public;

-- probe: schema-public-grant
-- v2:
grant create on schema public to gap_r1;

-- probe: schema-comment
-- base:
create schema s;
-- v2:
comment on schema s is 'stuff';

-- probe: schema-public-comment
-- v2:
comment on schema public is 'the default';

-- ===== sequences
-- probe: sequence-added
-- v2:
create sequence s;

-- probe: sequence-as
-- v1:
create sequence s;
-- v2:
create sequence s as int;

-- probe: sequence-increment
-- v1:
create sequence s;
-- v2:
create sequence s increment 5;

-- probe: sequence-minmax
-- v1:
create sequence s;
-- v2:
create sequence s minvalue 10 maxvalue 1000;

-- probe: sequence-start
-- v1:
create sequence s;
-- v2:
create sequence s start 100;

-- probe: sequence-cache
-- v1:
create sequence s;
-- v2:
create sequence s cache 20;

-- probe: sequence-cycle
-- v1:
create sequence s;
-- v2:
create sequence s cycle;

-- probe: sequence-owned-by
-- base:
create table t (id int);
create sequence s;
-- v2:
alter sequence s owned by t.id;

-- probe: sequence-unlogged
-- v1:
create sequence s;
-- v2:
create unlogged sequence s;

-- probe: sequence-owner
-- base:
create sequence s;
-- v2:
alter sequence s owner to gap_owner;

-- probe: sequence-grant
-- base:
create sequence s;
-- v2:
grant usage on sequence s to gap_r1;

-- probe: sequence-comment
-- base:
create sequence s;
-- v2:
comment on sequence s is 'ids';

-- probe: serial-sequence-grant
-- base:
create table t (id serial primary key);
-- v2:
grant usage on sequence t_id_seq to gap_r1;

-- ===== extended statistics
-- probe: statistics-added
-- base:
create table t (id int primary key, a int, b int);
-- v2:
create statistics st on a, b from t;

-- probe: statistics-kinds
-- base:
create table t (id int primary key, a int, b int);
-- v1:
create statistics st (ndistinct) on a, b from t;
-- v2:
create statistics st (dependencies) on a, b from t;

-- probe: statistics-expression
-- base:
create table t (id int primary key, a int, b int);
-- v1:
create statistics st on (a + 1), b from t;
-- v2:
create statistics st on (a + 2), b from t;

-- probe: statistics-target
-- base:
create table t (id int primary key, a int, b int);
create statistics st on a, b from t;
-- v2:
alter statistics st set statistics 500;

-- probe: statistics-owner
-- base:
create table t (id int primary key, a int, b int);
create statistics st on a, b from t;
-- v2:
alter statistics st owner to gap_owner;

-- probe: statistics-comment
-- base:
create table t (id int primary key, a int, b int);
create statistics st on a, b from t;
-- v2:
comment on statistics st is 'ab';

-- ===== tables: columns
-- probe: column-added
-- base:
create table t (id int primary key);
-- v2:
alter table t add column a int;

-- probe: column-type
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a bigint);

-- probe: column-typmod
-- v1:
create table t (id int primary key, a varchar(10));
-- v2:
create table t (id int primary key, a varchar(20));

-- probe: column-default
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int default 5);

-- probe: column-not-null
-- v1:
create table t (id int primary key, a int);
-- v2:
create table t (id int primary key, a int not null);

-- probe: column-not-null-named
-- v1:
create table t (id int primary key, a int not null);
-- v2:
create table t (id int primary key, a int constraint a_required not null);

-- probe: column-not-null-no-inherit
-- v1:
create table t (id int primary key, a int not null);
-- v2:
create table t (id int primary key, a int not null no inherit);

-- probe: column-not-null-not-valid
-- base:
create table t (id int primary key, a int);
-- v1:
alter table t add constraint a_nn not null a;
-- v2:
alter table t add constraint a_nn not null a not valid;

-- probe: column-collation
-- v1:
create table t (id int primary key, a text);
-- v2:
create table t (id int primary key, a text collate "C");

-- probe: column-storage
-- base:
create table t (id int primary key, a text);
-- v2:
alter table t alter column a set storage external;

-- probe: column-storage-inline
-- v1:
create table t (id int primary key, a text);
-- v2:
create table t (id int primary key, a text storage main);

-- probe: column-compression
-- v1:
create table t (id int primary key, a text);
-- v2:
create table t (id int primary key, a text compression lz4);

-- probe: column-statistics
-- base:
create table t (id int primary key, a int);
-- v2:
alter table t alter column a set statistics 500;

-- probe: column-n-distinct
-- base:
create table t (id int primary key, a int);
-- v2:
alter table t alter column a set (n_distinct = 100);

-- probe: column-n-distinct-inherited
-- base:
create table t (id int primary key, a int);
-- v2:
alter table t alter column a set (n_distinct_inherited = 100);

-- probe: column-generated-stored
-- v1:
create table t (id int primary key, a int generated always as (id * 2) stored);
-- v2:
create table t (id int primary key, a int generated always as (id * 3) stored);

-- probe: column-generated-virtual
-- v1:
create table t (id int primary key, a int generated always as (id * 2) stored);
-- v2:
create table t (id int primary key, a int generated always as (id * 2) virtual);

-- probe: column-identity-added
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key generated always as identity);

-- probe: column-identity-kind
-- v1:
create table t (id int primary key generated always as identity);
-- v2:
create table t (id int primary key generated by default as identity);

-- probe: column-identity-options
-- v1:
create table t (id int primary key generated always as identity);
-- v2:
create table t (id int primary key generated always as identity (start with 100 increment by 10));

-- probe: column-identity-cycle-cache
-- v1:
create table t (id int primary key generated always as identity);
-- v2:
create table t (id int primary key generated always as identity (cache 10 cycle));

-- probe: column-comment
-- base:
create table t (id int primary key, a int);
-- v2:
comment on column t.a is 'a';

-- probe: column-grant
-- base:
create table t (id int primary key, a int);
-- v2:
grant select (a) on t to gap_r1;

-- probe: column-grant-option
-- base:
create table t (id int primary key, a int);
-- v1:
grant update (a) on t to gap_r1;
-- v2:
grant update (a) on t to gap_r1 with grant option;

-- probe: column-renamed-case
-- v1:
create table t (id int primary key, "Name" text);
-- v2:
create table t (id int primary key, "name" text);

-- probe: column-array-dims
-- v1:
create table t (id int primary key, a int[]);
-- v2:
create table t (id int primary key, a int[][]);

-- probe: column-domain
-- base:
create domain d1 as int;
create domain d2 as int;
-- v1:
create table t (id int primary key, a d1);
-- v2:
create table t (id int primary key, a d2);

-- ===== tables: constraints
-- probe: check-added
-- base:
create table t (id int primary key, a int);
-- v2:
alter table t add constraint c check (a > 0);

-- probe: check-changed
-- v1:
create table t (id int primary key, a int, constraint c check (a > 0));
-- v2:
create table t (id int primary key, a int, constraint c check (a > 1));

-- probe: check-no-inherit
-- v1:
create table t (id int primary key, a int, constraint c check (a > 0));
-- v2:
create table t (id int primary key, a int, constraint c check (a > 0) no inherit);

-- probe: check-not-valid
-- base:
create table t (id int primary key, a int);
-- v1:
alter table t add constraint c check (a > 0);
-- v2:
alter table t add constraint c check (a > 0) not valid;

-- probe: check-not-enforced
-- v1:
create table t (id int primary key, a int, constraint c check (a > 0));
-- v2:
create table t (id int primary key, a int, constraint c check (a > 0) not enforced);

-- probe: check-comment
-- base:
create table t (id int primary key, a int, constraint c check (a > 0));
-- v2:
comment on constraint c on t is 'positive';

-- probe: unique-added
-- base:
create table t (id int primary key, a int);
-- v2:
alter table t add constraint u unique (a);

-- probe: unique-nulls-not-distinct
-- v1:
create table t (id int primary key, a int, constraint u unique (a));
-- v2:
create table t (id int primary key, a int, constraint u unique nulls not distinct (a));

-- probe: unique-include
-- v1:
create table t (id int primary key, a int, b int, constraint u unique (a));
-- v2:
create table t (id int primary key, a int, b int, constraint u unique (a) include (b));

-- probe: unique-fillfactor
-- v1:
create table t (id int primary key, a int, constraint u unique (a));
-- v2:
create table t (id int primary key, a int, constraint u unique (a) with (fillfactor = 50));

-- probe: unique-deferrable
-- v1:
create table t (id int primary key, a int, constraint u unique (a));
-- v2:
create table t (id int primary key, a int, constraint u unique (a) deferrable initially deferred);

-- probe: primary-key-columns
-- v1:
create table t (id int, a int, constraint pk primary key (id));
-- v2:
create table t (id int, a int, constraint pk primary key (id, a));

-- probe: primary-key-name
-- v1:
create table t (id int, constraint pk1 primary key (id));
-- v2:
create table t (id int, constraint pk2 primary key (id));

-- probe: primary-key-without-overlaps
-- base:
create extension btree_gist;
-- v1:
create table t (id int, valid tstzrange, constraint pk primary key (id, valid));
-- v2:
create table t (id int, valid tstzrange, constraint pk primary key (id, valid without overlaps));

-- probe: exclusion-added
-- base:
create extension btree_gist;
create table t (id int primary key, r int4range);
-- v2:
alter table t add constraint ex exclude using gist (r with &&);

-- probe: exclusion-where
-- base:
create extension btree_gist;
-- v1:
create table t (id int primary key, r int4range, constraint ex exclude using gist (r with &&));
-- v2:
create table t (id int primary key, r int4range, constraint ex exclude using gist (r with &&) where (id > 0));

-- probe: fk-added
-- base:
create table p (id int primary key);
create table c (id int primary key, p int);
-- v2:
alter table c add constraint fk foreign key (p) references p;

-- probe: fk-on-delete
-- base:
create table p (id int primary key);
-- v1:
create table c (id int primary key, p int, constraint fk foreign key (p) references p);
-- v2:
create table c (id int primary key, p int, constraint fk foreign key (p) references p on delete cascade);

-- probe: fk-on-delete-set-null-columns
-- base:
create table p (id int, k int, primary key (id, k));
-- v1:
create table c (id int primary key, p int, k int, constraint fk foreign key (p, k) references p on delete set null);
-- v2:
create table c (id int primary key, p int, k int, constraint fk foreign key (p, k) references p on delete set null (p));

-- probe: fk-on-update
-- base:
create table p (id int primary key);
-- v1:
create table c (id int primary key, p int, constraint fk foreign key (p) references p);
-- v2:
create table c (id int primary key, p int, constraint fk foreign key (p) references p on update cascade);

-- probe: fk-match-full
-- base:
create table p (id int, k int, primary key (id, k));
-- v1:
create table c (id int primary key, p int, k int, constraint fk foreign key (p, k) references p);
-- v2:
create table c (id int primary key, p int, k int, constraint fk foreign key (p, k) references p match full);

-- probe: fk-deferrable
-- base:
create table p (id int primary key);
-- v1:
create table c (id int primary key, p int, constraint fk foreign key (p) references p);
-- v2:
create table c (id int primary key, p int, constraint fk foreign key (p) references p deferrable initially deferred);

-- probe: fk-not-valid
-- base:
create table p (id int primary key);
create table c (id int primary key, p int);
-- v1:
alter table c add constraint fk foreign key (p) references p;
-- v2:
alter table c add constraint fk foreign key (p) references p not valid;

-- probe: fk-not-enforced
-- base:
create table p (id int primary key);
-- v1:
create table c (id int primary key, p int, constraint fk foreign key (p) references p);
-- v2:
create table c (id int primary key, p int, constraint fk foreign key (p) references p not enforced);

-- probe: fk-period
-- base:
create extension btree_gist;
create table p (id int, valid tstzrange, constraint ppk primary key (id, valid without overlaps));
-- v1:
create table c (id int, valid tstzrange);
-- v2:
create table c (id int, valid tstzrange, constraint fk foreign key (id, period valid) references p (id, period valid));

-- probe: fk-referenced-columns
-- base:
create table p (id int primary key, k int unique);
-- v1:
create table c (id int primary key, p int, constraint fk foreign key (p) references p (id));
-- v2:
create table c (id int primary key, p int, constraint fk foreign key (p) references p (k));

-- probe: constraint-index-tablespace-free-options
-- v1:
create table t (id int, constraint pk primary key (id));
-- v2:
create table t (id int, constraint pk primary key (id) with (fillfactor = 70));

-- ===== tables: storage and options
-- probe: table-unlogged
-- v1:
create table t (id int primary key);
-- v2:
create unlogged table t (id int primary key);

-- probe: table-fillfactor
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) with (fillfactor = 70);

-- probe: table-autovacuum
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) with (autovacuum_enabled = false, autovacuum_vacuum_scale_factor = 0.01);

-- probe: table-toast-options
-- v1:
create table t (id int primary key, a text);
-- v2:
create table t (id int primary key, a text) with (toast.autovacuum_enabled = false);

-- probe: table-replica-identity-full
-- base:
create table t (id int primary key, a int);
-- v2:
alter table t replica identity full;

-- probe: table-replica-identity-index
-- base:
create table t (id int primary key, a int not null);
create unique index ua on t (a);
-- v2:
alter table t replica identity using index ua;

-- probe: table-replica-identity-nothing
-- base:
create table t (id int primary key, a int);
-- v2:
alter table t replica identity nothing;

-- probe: table-owner
-- base:
create table t (id int primary key);
-- v2:
alter table t owner to gap_owner;

-- probe: table-grant
-- base:
create table t (id int primary key);
-- v2:
grant select, insert on t to gap_r1;

-- probe: table-grant-option
-- base:
create table t (id int primary key);
-- v1:
grant select on t to gap_r1;
-- v2:
grant select on t to gap_r1 with grant option;

-- probe: table-revoke-owner
-- base:
create table t (id int primary key);
-- v2:
revoke delete on t from current_user;

-- probe: table-grant-public
-- base:
create table t (id int primary key);
-- v2:
grant select on t to public;

-- probe: table-granted-by
-- base:
create table t (id int primary key);
alter table t owner to gap_owner;
-- v1:
grant select on t to gap_r1;
-- v2:
grant select on t to gap_r1 with grant option;
set role gap_r1;
grant select on t to gap_r2;
reset role;

-- probe: table-comment
-- base:
create table t (id int primary key);
-- v2:
comment on table t is 'things';

-- probe: table-of-type
-- base:
create type ty as (a int, b text);
-- v1:
create table t (a int, b text);
-- v2:
create table t of ty;

-- probe: table-inherits
-- base:
create table p (id int, a int);
-- v1:
create table c (id int, a int, b int);
-- v2:
create table c (b int) inherits (p);

-- probe: table-inherits-local-default
-- base:
create table p (id int, a int);
-- v1:
create table c (b int) inherits (p);
-- v2:
create table c (b int) inherits (p);
alter table c alter column a set default 9;

-- probe: table-partition-bound
-- base:
create table p (id int) partition by range (id);
-- v1:
create table p1 partition of p for values from (0) to (10);
-- v2:
create table p1 partition of p for values from (0) to (20);

-- probe: table-partition-default
-- base:
create table p (id int) partition by list (id);
-- v2:
create table pd partition of p default;

-- probe: table-partition-key
-- v1:
create table p (id int, a int) partition by range (id);
-- v2:
create table p (id int, a int) partition by range (a);

-- probe: table-partition-key-opclass
-- v1:
create table p (id text) partition by range (id);
-- v2:
create table p (id text) partition by range (id collate "C");

-- probe: table-subpartition
-- base:
create table p (id int, a int) partition by range (id);
-- v1:
create table p1 partition of p for values from (0) to (10);
-- v2:
create table p1 partition of p for values from (0) to (10) partition by list (a);

-- probe: table-partition-own-default
-- base:
create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
-- v2:
alter table p1 alter column a set default 3;

-- probe: table-partition-own-index
-- base:
create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
-- v2:
create index p1a on p1 (a);

-- probe: table-partition-owner
-- base:
create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
-- v2:
alter table p1 owner to gap_owner;

-- probe: table-partition-grant
-- base:
create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
-- v2:
grant select on p1 to gap_r1;

-- probe: table-partition-rls
-- base:
create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
-- v2:
alter table p1 enable row level security;

-- probe: table-stops-inheriting
-- base:
create table p (id int, a int);
create table c (b int) inherits (p);
-- v2:
alter table c no inherit p;

-- ===== triggers
-- probe: trigger-added
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
-- v2:
create trigger tg before insert on t for each row execute function tf();

-- probe: trigger-timing
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
-- v1:
create trigger tg before insert on t for each row execute function tf();
-- v2:
create trigger tg after insert on t for each row execute function tf();

-- probe: trigger-update-of
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
-- v1:
create trigger tg before update on t for each row execute function tf();
-- v2:
create trigger tg before update of a on t for each row execute function tf();

-- probe: trigger-when
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
-- v1:
create trigger tg before update on t for each row execute function tf();
-- v2:
create trigger tg before update on t for each row when (old.a is distinct from new.a) execute function tf();

-- probe: trigger-arguments
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
-- v1:
create trigger tg before insert on t for each row execute function tf('x');
-- v2:
create trigger tg before insert on t for each row execute function tf('y');

-- probe: trigger-transition-table
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return null; end $$;
-- v1:
create trigger tg after insert on t for each statement execute function tf();
-- v2:
create trigger tg after insert on t referencing new table as added for each statement execute function tf();

-- probe: trigger-constraint-deferred
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return null; end $$;
-- v1:
create constraint trigger tg after insert on t for each row execute function tf();
-- v2:
create constraint trigger tg after insert on t deferrable initially deferred for each row execute function tf();

-- probe: trigger-disabled
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
create trigger tg before insert on t for each row execute function tf();
-- v2:
alter table t disable trigger tg;

-- probe: trigger-enable-replica
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
create trigger tg before insert on t for each row execute function tf();
-- v2:
alter table t enable replica trigger tg;

-- probe: trigger-comment
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
create trigger tg before insert on t for each row execute function tf();
-- v2:
comment on trigger tg on t is 'stamp';

-- probe: trigger-depends-on-extension
-- base:
create extension citext;
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
create trigger tg before insert on t for each row execute function tf();
-- v2:
alter trigger tg on t depends on extension citext;

-- probe: trigger-on-view
-- base:
create table t (id int primary key, a int);
create view v as select * from t;
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
-- v2:
create trigger tg instead of insert on v for each row execute function tf();

-- probe: trigger-on-partitioned
-- base:
create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
-- v2:
create trigger tg before insert on p for each row execute function tf();

-- ===== types
-- probe: enum-added
-- v2:
create type e as enum ('a', 'b');

-- probe: enum-value-added
-- v1:
create type e as enum ('a', 'b');
-- v2:
create type e as enum ('a', 'b', 'c');

-- probe: enum-value-renamed
-- v1:
create type e as enum ('a', 'b');
-- v2:
create type e as enum ('a', 'c');

-- probe: enum-order
-- v1:
create type e as enum ('a', 'b');
-- v2:
create type e as enum ('b', 'a');

-- probe: composite-added
-- v2:
create type c as (a int, b text);

-- probe: composite-attribute-type
-- v1:
create type c as (a int, b text);
-- v2:
create type c as (a bigint, b text);

-- probe: composite-attribute-collation
-- v1:
create type c as (a int, b text);
-- v2:
create type c as (a int, b text collate "C");

-- probe: composite-attribute-order
-- v1:
create type c as (a int, b text);
-- v2:
create type c as (b text, a int);

-- probe: composite-attribute-comment
-- base:
create type c as (a int, b text);
-- v2:
comment on column c.a is 'a';

-- probe: range-added
-- v2:
create type r as range (subtype = int4);

-- probe: range-subtype-diff
-- v1:
create type r as range (subtype = float8);
-- v2:
create type r as range (subtype = float8, subtype_diff = float8mi);

-- probe: range-multirange-name
-- v1:
create type r as range (subtype = int4);
-- v2:
create type r as range (subtype = int4, multirange_type_name = rs);

-- probe: range-collation
-- v1:
create type r as range (subtype = text);
-- v2:
create type r as range (subtype = text, collation = "C");

-- probe: type-owner
-- base:
create type e as enum ('a');
-- v2:
alter type e owner to gap_owner;

-- probe: type-comment
-- base:
create type e as enum ('a');
-- v2:
comment on type e is 'e';

-- probe: type-grant
-- base:
create type e as enum ('a');
-- v2:
revoke usage on type e from public;

-- probe: shell-type
-- v2:
create type sh;

-- probe: base-type-storage
-- base:
create type bt;
create function bt_in(cstring) returns bt language internal immutable strict as 'int4in';
create function bt_out(bt) returns cstring language internal immutable strict as 'int4out';
-- v1:
create type bt (input = bt_in, output = bt_out, internallength = 4, passedbyvalue, alignment = int4);
-- v2:
create type bt (input = bt_in, output = bt_out, internallength = 4, passedbyvalue, alignment = int4, category = 'N');

-- ===== views
-- probe: view-added
-- v2:
create view v as select 1 as a;

-- probe: view-query
-- v1:
create view v as select 1 as a;
-- v2:
create view v as select 2 as a;

-- probe: view-security-barrier
-- v1:
create view v as select 1 as a;
-- v2:
create view v with (security_barrier) as select 1 as a;

-- probe: view-security-invoker
-- v1:
create view v as select 1 as a;
-- v2:
create view v with (security_invoker = true) as select 1 as a;

-- probe: view-check-option
-- base:
create table t (id int primary key, a int);
-- v1:
create view v as select * from t where a > 0;
-- v2:
create view v as select * from t where a > 0 with local check option;

-- probe: view-column-default
-- base:
create table t (id int primary key, a int);
create view v as select * from t;
-- v2:
alter view v alter column a set default 1;

-- probe: view-column-names
-- v1:
create view v (a) as select 1;
-- v2:
create view v (b) as select 1;

-- probe: view-owner
-- base:
create view v as select 1 as a;
-- v2:
alter view v owner to gap_owner;

-- probe: view-grant
-- base:
create view v as select 1 as a;
-- v2:
grant select on v to gap_r1;

-- probe: view-column-grant
-- base:
create view v as select 1 as a, 2 as b;
-- v2:
grant select (a) on v to gap_r1;

-- probe: view-comment
-- base:
create view v as select 1 as a;
-- v2:
comment on view v is 'v';

-- probe: view-column-comment
-- base:
create view v as select 1 as a;
-- v2:
comment on column v.a is 'a';

-- probe: view-recursive
-- v1:
create view v as select 1 as n;
-- v2:
create recursive view v (n) as select 1 union all select n + 1 from v where n < 3;

-- ===== casts
-- probe: cast-added
-- base:
create type c as (a int);
create function c_to_int(c) returns int language sql immutable as 'select ($1).a';
-- v2:
create cast (c as int) with function c_to_int(c);

-- probe: cast-context
-- base:
create type c as (a int);
create function c_to_int(c) returns int language sql immutable as 'select ($1).a';
-- v1:
create cast (c as int) with function c_to_int(c);
-- v2:
create cast (c as int) with function c_to_int(c) as implicit;

-- probe: cast-comment
-- base:
create type c as (a int);
create function c_to_int(c) returns int language sql immutable as 'select ($1).a';
create cast (c as int) with function c_to_int(c);
-- v2:
comment on cast (c as int) is 'cast';

-- ===== large objects
-- probe: large-object
-- v2:
select lo_create(424242);
grant select on large object 424242 to gap_r1;

-- ===== more
-- probe: default-privileges-large-objects
-- v2:
alter default privileges grant select on large objects to gap_r1;

-- probe: table-grant-maintain
-- base:
create table t (id int primary key);
-- v2:
grant maintain on t to gap_r1;

-- probe: function-set-from-current
-- v1:
create function f() returns int language sql as 'select 1';
-- v2:
create function f() returns int language sql set work_mem = '8MB' set statement_timeout = '5s' as 'select 1';

-- probe: function-plpgsql-volatility
-- v1:
create function f() returns int language plpgsql as $$ begin return 1; end $$;
-- v2:
create function f() returns int language plpgsql stable as $$ begin return 1; end $$;

-- probe: function-out-params
-- v1:
create function f(out a int) language sql as 'select 1';
-- v2:
create function f(out a int, out b int) language sql as 'select 1, 2';

-- probe: function-variadic
-- v1:
create function f(a int[]) returns int language sql as 'select 1';
-- v2:
create function f(variadic a int[]) returns int language sql as 'select 1';

-- probe: column-drop-expression
-- v1:
create table t (id int primary key, a int generated always as (id * 2) stored);
-- v2:
create table t (id int primary key, a int);

-- probe: sequence-as-smallint
-- v1:
create sequence s as int;
-- v2:
create sequence s as smallint;

-- probe: table-partition-hash
-- base:
create table p (id int) partition by hash (id);
-- v1:
create table p0 partition of p for values with (modulus 2, remainder 0);
-- v2:
create table p0 partition of p for values with (modulus 4, remainder 0);

-- probe: trigger-cloned-disabled-on-partition
-- base:
create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
create trigger tg before insert on p for each row execute function tf();
-- v2:
alter table p1 disable trigger tg;

-- probe: fk-to-partitioned
-- base:
create table p (id int primary key) partition by range (id);
create table p1 partition of p for values from (0) to (10);
create table c (id int primary key, p int);
-- v2:
alter table c add constraint fk foreign key (p) references p;

-- probe: event-trigger-enable-replica
-- base:
create function et() returns event_trigger language plpgsql as $$ begin end $$;
create event trigger e on ddl_command_end execute function et();
-- v2:
alter event trigger e enable replica;

-- probe: foreign-table-column-comment
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
create foreign table ft (a int) server s;
-- v2:
comment on column ft.a is 'a';

-- probe: foreign-table-column-statistics
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
create foreign table ft (a int) server s;
-- v2:
alter foreign table ft alter column a set statistics 200;

-- probe: operator-class-comment
-- base:
create function cmpi(int, int) returns int language sql immutable as 'select case when $1 < $2 then -1 when $1 > $2 then 1 else 0 end';
create operator class myops for type int using btree as operator 1 <, operator 2 <=, operator 3 =, operator 4 >=, operator 5 >, function 1 cmpi(int, int);
-- v2:
comment on operator class myops using btree is 'mine';

-- probe: table-reloptions-parallel
-- v1:
create table t (id int primary key);
-- v2:
create table t (id int primary key) with (parallel_workers = 4, toast_tuple_target = 256);

-- probe: view-security-barrier-false
-- v1:
create view v as select 1 as a;
-- v2:
create view v with (security_barrier = false) as select 1 as a;

-- probe: index-brin-autosummarize
-- base:
create table t (id int primary key, a int);
-- v1:
create index i on t using brin (a);
-- v2:
create index i on t using brin (a) with (autosummarize = on);

-- probe: table-access-method-default
-- base:
create access method myheap type table handler heap_tableam_handler;
create table t (id int) using myheap;
-- v2:
alter table t set access method heap;

-- probe: enum-empty
-- v1:
create type e as enum ();
-- v2:
create type e as enum ('a');

-- probe: schema-default-acl-in-schema-functions
-- base:
create schema s;
-- v2:
alter default privileges in schema s grant execute on functions to gap_r1;

-- probe: column-identity-generated-change
-- base:
create table t (id int primary key generated always as identity);
-- v2:
alter table t alter column id set generated by default;

-- probe: constraint-trigger-from
-- base:
create table p (id int primary key);
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return null; end $$;
-- v1:
create constraint trigger tg after insert on t for each row execute function tf();
-- v2:
create constraint trigger tg after insert on t from p for each row execute function tf();

-- probe: comment-on-column-of-composite-quoted
-- base:
create type "C" as ("A" int);
-- v2:
comment on column "C"."A" is 'x';

-- probe: rule-instead-on-view
-- base:
create table t (id int primary key, a int);
create view v as select * from t;
-- v2:
create rule r as on insert to v do instead insert into t values (new.id, new.a);

-- probe: range-subtype-opclass
-- v1:
create type r as range (subtype = text, collation = "C");
-- v2:
create type r as range (subtype = text, collation = "C", subtype_opclass = text_pattern_ops);

-- probe: schema-public-owner
-- v2:
alter schema public owner to gap_owner;

-- probe: schema-public-revoke-usage
-- v2:
revoke usage on schema public from public;

-- probe: not-null-comment
-- base:
create table t (id int primary key, a int not null);
-- v2:
comment on constraint t_a_not_null on t is 'required';

-- probe: text-search-parser-added
-- v2:
create text search parser p (start = prsd_start, gettoken = prsd_nexttoken, end = prsd_end, lextypes = prsd_lextype);

-- probe: cast-inout
-- base:
create type c as (a int);
-- v1:
create cast (c as text) with inout;
-- v2:
create cast (c as text) with inout as assignment;

-- probe: function-reset
-- v1:
create function f() returns int language sql set work_mem = '8MB' as 'select 1';
-- v2:
create function f() returns int language sql as 'select 1';

-- probe: sequence-owned-by-none
-- base:
create table t (id int);
-- v1:
create sequence s owned by t.id;
-- v2:
create sequence s;

-- probe: table-not-of
-- base:
create type ty as (a int, b text);
-- v1:
create table t of ty;
-- v2:
create table t of ty;
alter table t not of;

-- probe: identity-sequence-grant
-- base:
create table t (id int generated always as identity primary key);
-- v2:
grant usage on sequence t_id_seq to gap_r1;

-- probe: extension-function-owner
-- base:
create extension pgcrypto;
-- v2:
alter function digest(text, text) owner to gap_owner;

-- probe: extension-table-default-privileges
-- base:
create extension postgres_fdw;
create server s foreign data wrapper postgres_fdw;
-- v2:
grant usage on foreign server s to gap_r1;
grant usage on foreign data wrapper postgres_fdw to gap_r2;

-- probe: base-type-element
-- base:
create type bt;
create function bt_in(cstring) returns bt language internal immutable strict as 'int4in';
create function bt_out(bt) returns cstring language internal immutable strict as 'int4out';
-- v1:
create type bt (input = bt_in, output = bt_out, internallength = 4, passedbyvalue, alignment = int4);
-- v2:
create type bt (input = bt_in, output = bt_out, internallength = 4, passedbyvalue, alignment = int4, element = int2);

-- probe: base-type-subscript
-- base:
create type bt;
create function bt_in(cstring) returns bt language internal immutable strict as 'int4in';
create function bt_out(bt) returns cstring language internal immutable strict as 'int4out';
-- v1:
create type bt (input = bt_in, output = bt_out, internallength = 4, passedbyvalue, alignment = int4);
-- v2:
create type bt (input = bt_in, output = bt_out, internallength = 4, passedbyvalue, alignment = int4, element = int2, subscript = raw_array_subscript_handler);

-- probe: trigger-truncate
-- base:
create table t (id int primary key, a int);
create function tf() returns trigger language plpgsql as $$ begin return null; end $$;
-- v1:
create trigger tg after delete on t for each statement execute function tf();
-- v2:
create trigger tg after delete or truncate on t for each statement execute function tf();

-- probe: exclusion-deferrable
-- base:
create extension btree_gist;
-- v1:
create table t (id int primary key, r int4range, constraint ex exclude using gist (r with &&));
-- v2:
create table t (id int primary key, r int4range, constraint ex exclude using gist (r with &&) deferrable initially deferred);

-- probe: constraint-index-comment
-- base:
create table t (id int, constraint pk primary key (id));
-- v2:
comment on index pk is 'the key';

-- probe: identity-sequence-comment
-- base:
create table t (id int generated always as identity primary key);
-- v2:
comment on sequence t_id_seq is 'ids';

-- probe: policy-on-partitioned
-- base:
create table p (id int, owner text) partition by range (id);
create table p1 partition of p for values from (0) to (10);
-- v2:
alter table p enable row level security;
create policy pol on p using (owner = current_user);

-- probe: column-generated-identity-on-partition
-- v1:
create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
-- v2:
create table p (id int generated always as identity, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);

-- probe: function-security-invoker-explicit
-- v1:
create function f() returns int language sql security definer as 'select 1';
-- v2:
create function f() returns int language sql security definer set search_path = '' as 'select 1';

-- probe: aggregate-ordered-set
-- v1:
create aggregate my_pct(float8 order by float8) (sfunc = ordered_set_transition, stype = internal, finalfunc = percentile_disc_final, finalfunc_extra);
-- v2:
create aggregate my_pct(float8 order by float8) (sfunc = ordered_set_transition, stype = internal, finalfunc = percentile_disc_final, finalfunc_extra, finalfunc_modify = shareable);

-- probe: identity-sequence-logged
-- base:
create unlogged table t (id bigint generated always as identity primary key);
-- v2:
alter sequence t_id_seq set logged;

-- probe: serial-sequence-unlogged
-- base:
create table t (id serial primary key);
-- v2:
alter sequence t_id_seq set unlogged;
