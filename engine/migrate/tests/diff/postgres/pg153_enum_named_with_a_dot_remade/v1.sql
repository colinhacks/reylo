create type "mood.x" as enum ('a', 'b');
create table t (id integer primary key, m "mood.x");
