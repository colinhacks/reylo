create table t (id int not null, [order] int not null, [first name] nvarchar(40) null, constraint t_pk primary key nonclustered (id));
create clustered index t_order on t ([order]);
