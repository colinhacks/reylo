create domain d as text not null default '1';
comment on domain d is 'a code';
create function show(x d) returns text language sql immutable as 'select x::text';
create table t (id integer primary key, c d);
create view v as select id, c from t;
