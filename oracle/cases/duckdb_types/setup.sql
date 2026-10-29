-- Each DuckDB type a column, a parameter and an expression can have, as the engine names it.
create type mood as enum ('happy', 'sad');
create table everything (
  id integer primary key, flag boolean, tiny tinyint, small smallint, big bigint, huge hugeint, ubig ubigint, real_value float, double_value double,
  money decimal(18, 3), body varchar, bytes blob, uid uuid, day date, seen timestamp, seen_tz timestamptz, seen_ns timestamp_ns, clock time, span interval,
  doc json, tags varchar[], vector float[3], meta struct(host varchar, pid integer), counts map(varchar, integer), either union(num integer, str varchar), feeling mood
);
create table nested (id integer primary key, shape struct(a integer, "b c" varchar) not null, pairs map(varchar, integer) not null, choice union(num integer, str varchar) not null, fixed integer[2] not null);
