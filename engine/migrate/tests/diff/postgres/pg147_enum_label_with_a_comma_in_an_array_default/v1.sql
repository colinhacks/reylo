create type mood as enum ('a');
create table t (id integer primary key, m mood[]);
