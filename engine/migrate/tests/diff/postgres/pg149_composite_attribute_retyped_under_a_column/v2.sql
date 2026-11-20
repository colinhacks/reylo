create type p0 as (a bigint, b text);
create table t (id integer primary key, c p0 default row(0, 'x')::p0);
