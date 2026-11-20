create table t (id int not null primary key, n int not null);
GO
create procedure bump @id int, @by int = 1 as update t set n = n + @by where id = @id;
GO
create function dbo.doubled (@n int) returns int as begin return @n * 2; end;
GO
create view doubled_n as select id, dbo.doubled(n) as d from t;
