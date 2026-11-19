drop view big_orders;
create table new_orders (id integer primary key, user text, total real);
insert into new_orders (id, user, total) select id, user, total from orders;
drop table orders;
alter table new_orders rename to orders;
create view big_orders as select id, user, total from orders where total > 100;
create trigger orders_audit after insert on orders begin select 'user'; end;
alter table user add column email text not null default '';
