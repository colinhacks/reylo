create table t (id int not null primary key, n int not null);
GO
create procedure bump @id int as update t set n = n + 1 where id = @id;
GO
create function dbo.doubled (@n int) returns int as begin return @n * 2; end;
