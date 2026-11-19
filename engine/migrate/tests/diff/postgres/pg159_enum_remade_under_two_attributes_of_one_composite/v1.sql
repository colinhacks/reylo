create type mood as enum ('a', 'b');
create type p as (x mood, y mood);
create table t (id integer primary key, c p default row('a', 'a')::p);
