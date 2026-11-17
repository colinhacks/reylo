with input as (select :name::nvarchar(100) as name) select c.id from customers c, input where (input.name is null or c.name = input.name)
