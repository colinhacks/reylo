create type mood as enum ('a', 'c', 'b');
create type p as (a mood);
create table t (id integer primary key, c text);
