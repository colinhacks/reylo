create table f (
  id int not null primary key,
  n int not null
);
create table g2 (
  id int identity(1, 1) not null primary key,
  x int not null
);
GO
create view vj as select f.id as fid, g2.id as gid, g2.x from f left join g2 on g2.id = f.id;
GO
create view vu as select f.id, f.n, cast(null as int) as z from f;
