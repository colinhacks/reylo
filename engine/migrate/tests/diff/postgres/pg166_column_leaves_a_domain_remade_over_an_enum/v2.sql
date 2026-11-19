create type mood as enum ('a');
create domain d as mood;
create table t (id integer primary key, c mood);
