create type p as (a numeric, b bigint, x integer);
create table "T" (id integer not null, c p default row(7.5, 7, 7)::p, primary key (id)) partition by range (id);
create table "T_p0" partition of "T" for values from (1) to (4);
