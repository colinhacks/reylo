create type mood as enum ('a', 'b'); create type by_day as map(varchar, mood); create table t (id integer primary key, m by_day);
