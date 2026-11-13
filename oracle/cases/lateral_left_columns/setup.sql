-- A left-joined LATERAL subquery's column is NULL wherever the subquery finds no row, a column
-- it reads from the table before it included: the join nulls the item, whatever its columns read.
create table orders (id int primary key, customer text not null);
create table items (id int primary key, order_id int not null references orders (id), sku text not null);
