create type "mood.x" as enum ('a');
create table t (id integer primary key, m "mood.x");
