create type mood as enum ('a', 'b'); create table t (id integer primary key, m varchar); create index t_m on t ((m::mood));
