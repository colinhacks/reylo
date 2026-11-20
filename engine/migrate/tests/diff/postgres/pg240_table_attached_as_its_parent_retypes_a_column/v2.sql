create table p (id int not null, c timestamp) partition by list (id);
create table x partition of p for values in (1);
