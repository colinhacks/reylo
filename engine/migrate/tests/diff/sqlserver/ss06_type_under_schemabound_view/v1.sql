create table orders (id int not null primary key, total decimal(10, 2) not null);
GO
create view dbo.order_totals with schemabinding as select id, total from dbo.orders;
