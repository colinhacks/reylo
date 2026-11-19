create table user (id integer primary key, name text not null, email text not null default '');
create table orders (id integer primary key, user text, total real);
create view big_orders as select id, user, total from orders where total > 100;
create trigger orders_audit after insert on orders begin select 'user'; end;
