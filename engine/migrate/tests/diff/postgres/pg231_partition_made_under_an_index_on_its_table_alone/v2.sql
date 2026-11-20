create table p (id int not null, c int) partition by list (id);
create table x partition of p for values in (101);
create index i on only p (c);
