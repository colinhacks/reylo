create table f (
  id int not null primary key,
  n int not null,
  s nvarchar(200) not null,
  v varchar(10) not null,
  d datetime2(7) not null,
  m int null
);
create table g (
  id int not null primary key,
  f_id int not null,
  x int not null
);
create table cc (
  id int not null primary key,
  n int not null,
  s nvarchar(200) not null,
  c_choose as (choose(n, 1, 2)),
  c_space as (space(n)),
  c_char as (char(n)),
  c_stuff as (stuff(s, n, 1, N'x')),
  c_quotename as (quotename(s)),
  c_isnull as (isnull(nullif(n, 0), n)),
  c_nullif as (nullif(n, 0))
);
create table base (
  id int not null primary key,
  x int not null
);
GO
create function dbo.upper(@s nvarchar(200)) returns nvarchar(200) as begin return null end
GO
create function dbo.count(@x int) returns int as begin return null end
GO
create function dbo.max(@x int) returns int as begin return null end
GO
create function dbo.isnull(@x int, @y int) returns int as begin return null end
GO
create function dbo.concat(@x int, @y int) returns int as begin return null end
GO
create function dbo.newid() returns int as begin return null end
GO
create function dbo.substring(@s nvarchar(200), @a int, @b int) returns nvarchar(200) as begin return null end
GO
create view vb as select id, x from base;
GO
create function dbo.tvb() returns table as return select id, x from base;
GO
alter table base alter column x int null;
GO
create table js (
  id int not null primary key,
  j nvarchar(100) not null check (j in (N'[null, 1]', N'{"a": null, "b": 2}'))
);
