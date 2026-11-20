declare @n nvarchar(100) = :name
select id from customers where (@n is null or name = @n)
