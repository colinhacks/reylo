create type mood as enum ('a', 'b', 'c'); create table t (id integer primary key, m varchar); create index t_m on t ((m::mood));
