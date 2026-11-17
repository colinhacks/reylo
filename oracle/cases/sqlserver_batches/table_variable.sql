declare @ids table (id int not null primary key)
insert into @ids (id) select id from orders where total > @min
select o.id, o.total from orders o join @ids i on i.id = o.id
