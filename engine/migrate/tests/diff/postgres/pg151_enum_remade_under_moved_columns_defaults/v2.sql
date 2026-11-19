create type mood as enum ('a', 'c');
create type p as (a mood);
create domain d as mood;
create table t (id integer primary key, c p default row('a')::p, n d default 'c');
