create type mood as enum ('a', 'b');
create type p0 as (a mood, b bigint);
create table t (id integer not null, c p0, primary key (id)) partition by list (id);
create table t_p0 partition of t for values in (1);
