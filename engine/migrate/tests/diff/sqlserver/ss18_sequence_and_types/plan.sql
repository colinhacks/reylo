create type email from nvarchar(320) not null;
create type id_list as table (
  id int not null,
  primary key clustered (id)
);
create sequence order_numbers as bigint start with 1000 increment by 1 minvalue -9223372036854775808 maxvalue 9223372036854775807 no cycle cache;
alter table t add contact email null;
GO
create procedure touch_all @ids dbo.id_list readonly as select id from @ids;
