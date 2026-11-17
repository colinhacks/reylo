declare @n int
select @n = count(*) from orders where total > @min
set @n = @n + 1
select @n as n
