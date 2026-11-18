create table t (id integer, v integer default 2) partition by range (id);
create table p partition of t (v default 5) for values from (0) to (10);
create table q partition of t for values from (10) to (20);
