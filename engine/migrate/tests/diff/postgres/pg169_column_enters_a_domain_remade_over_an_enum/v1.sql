create type mood as enum ('a', 'b');
create domain d as mood;
create table t (id integer primary key, c mood);
