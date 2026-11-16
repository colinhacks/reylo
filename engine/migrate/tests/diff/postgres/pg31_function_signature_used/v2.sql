create function tax(amount numeric, rate numeric) returns numeric language sql immutable as 'select amount * rate';
create table orders (id integer primary key, amount numeric not null, fee numeric default tax(10, 0.2));
create view order_tax as select id, tax(amount, 0.2) as tax from orders;
