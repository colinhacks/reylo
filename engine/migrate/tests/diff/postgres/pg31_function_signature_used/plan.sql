drop view order_tax;
alter table orders alter column fee drop default;
drop function tax(numeric);
create function tax(amount numeric, rate numeric) returns numeric language sql immutable as 'select amount * rate';
alter table orders alter column fee set default tax(10, 0.2);
create view order_tax as select id, tax(amount, 0.2) as tax from orders;
