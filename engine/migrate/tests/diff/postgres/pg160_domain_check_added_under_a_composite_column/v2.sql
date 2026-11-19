create domain d as integer constraint k check (value > 0);
create type p as (a d);
create table t (id integer primary key, c p);
