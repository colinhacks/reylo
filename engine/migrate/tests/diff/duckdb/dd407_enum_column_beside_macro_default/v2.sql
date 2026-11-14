create type mood as enum ('a', 'b', 'c'); create macro m(x) as x + 2; create table t (a integer default m(1), c mood);
