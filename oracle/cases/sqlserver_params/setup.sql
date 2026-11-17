create table customers (
  id int not null primary key,
  email nvarchar(200) not null unique,
  name nvarchar(100) null,
  vip bit not null default 0,
  credit money null,
  joined datetime2(3) not null default sysutcdatetime()
);
create table orders (
  id bigint identity(1, 1) not null primary key,
  customer_id int not null references customers (id),
  status varchar(10) not null check (status in ('open', 'paid', 'void')),
  total decimal(12, 2) not null,
  note nvarchar(max) null,
  placed datetimeoffset(3) not null,
  shipped date null
);
create table lines (
  order_id bigint not null references orders (id),
  n int not null,
  sku char(8) not null,
  qty smallint not null,
  price decimal(10, 2) not null,
  discount decimal(5, 4) null,
  primary key (order_id, n)
);
