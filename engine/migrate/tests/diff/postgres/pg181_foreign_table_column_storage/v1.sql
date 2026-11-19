create foreign data wrapper w;
create server s foreign data wrapper w;
create foreign table ft (code char(5) not null) server s;
create table t (id int);
