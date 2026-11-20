create table users (
  id int not null primary key,
  name nvarchar(40) not null,
  nick nvarchar(20) null,
  active bit not null
);
create table posts (
  id int not null primary key,
  user_id int not null references users (id),
  title nvarchar(80) not null,
  body nvarchar(max) null,
  score decimal(10, 2) not null,
  at datetime2(3) not null,
  tag uniqueidentifier null
);
insert into users values (1, N'Ada', null, 1), (2, N'Grace', N'g', 0), (3, N'Edsger', null, 1);
insert into posts values
  (1, 1, N'First', N'hello', 1.50, '2026-01-01T00:00:00', newid()),
  (2, 1, N'Second', null, 0, '2026-02-01T12:30:00.123', null),
  (3, 2, N'Third', N'x', -2.25, '2026-03-01', null);
