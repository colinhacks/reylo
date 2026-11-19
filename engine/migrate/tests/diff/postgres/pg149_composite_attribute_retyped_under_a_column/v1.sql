create type p0 as (a integer, b text);
create table t (id integer primary key, c p0 default row(0, 'x')::p0);
