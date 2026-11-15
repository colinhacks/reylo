create type mood as enum ('a', 'b'); create macro m(x) as x + 1; create table t (a integer default m(1), c mood);
