declare @low decimal(10, 2) = @min, @high decimal(10, 2) = 1000.00
select id, total from orders where total between @low and @high
