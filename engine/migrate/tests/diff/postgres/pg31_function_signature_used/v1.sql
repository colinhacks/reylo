create function tax(amount numeric) returns numeric language sql immutable as 'select amount * 0.2';
create table orders (id integer primary key, amount numeric not null, fee numeric default tax(10));
create view order_tax as select id, tax(amount) as tax from orders;
