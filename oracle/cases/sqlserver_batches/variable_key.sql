declare @key int = @id
select id, total from orders where id = @key
