create type mood as enum ('a', 'b', 'c');
create type p as (a mood);
create table t (id integer primary key, c p);
