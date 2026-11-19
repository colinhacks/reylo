create table "t x" (id int not null) partition by range (id);
create table x0 partition of "t x" for values from (1) to (4);
