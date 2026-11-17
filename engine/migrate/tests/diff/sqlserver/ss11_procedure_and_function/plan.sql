create view doubled_n as select id, dbo.doubled(n) as d from t;
GO
create or alter procedure bump @id int, @by int = 1 as update t set n = n + @by where id = @id;
