create table orders (
  id int identity(1, 1) not null primary key,
  total decimal(10, 2) not null,
  note nvarchar(50) null,
  placed_at datetime2(3) not null
);
insert into orders (total, note, placed_at) values (10.50, N'first', '2026-01-01T00:00:00'), (0, null, '2026-06-01T12:00:00');
GO
create procedure dbo.order_totals @min decimal(10, 2) as select id, total from orders where total >= @min order by id;
