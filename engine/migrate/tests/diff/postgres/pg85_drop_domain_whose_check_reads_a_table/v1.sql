create table kept (id integer primary key);
create table t (id integer primary key);
create function g(x integer) returns boolean language sql immutable begin atomic select x >= (select count(*)::integer from t where false); end;
create domain d as integer check (g(value));
alter table t add column c d;
