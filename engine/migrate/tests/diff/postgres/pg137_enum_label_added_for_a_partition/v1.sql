create type region as enum ('eu', 'us');
create table t (id integer, r region) partition by list (r);
create table t_eu partition of t for values in ('eu');
create table t_us partition of t for values in ('us');
