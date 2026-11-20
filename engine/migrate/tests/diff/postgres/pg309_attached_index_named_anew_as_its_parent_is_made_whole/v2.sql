create table p (id int) partition by list (id);
create table h partition of p for values in (1);
create table j partition of p for values in (2);
create index new_child on h (id);
create index pi on p (id);
