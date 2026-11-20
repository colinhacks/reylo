create sequence order_numbers as bigint start with 1000 increment by 1;
GO
create type dbo.email from nvarchar(320) not null;
GO
create type dbo.id_list as table (id int not null primary key);
GO
create table t (id int not null primary key, contact dbo.email null);
GO
create procedure touch_all @ids dbo.id_list readonly as select id from @ids;
