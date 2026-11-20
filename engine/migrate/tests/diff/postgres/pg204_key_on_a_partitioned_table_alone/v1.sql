create table p (a int not null) partition by list (a);
create table p0 partition of p for values in (0);
alter table p0 add primary key (a);
