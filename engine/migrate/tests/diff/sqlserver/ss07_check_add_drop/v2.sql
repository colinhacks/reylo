create table products (id int not null primary key, price decimal(10, 2) not null, stock int not null, constraint stock_positive check (stock >= 0));
