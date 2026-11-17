create table products (id int not null primary key, price decimal(10, 2) not null check (price > 0), stock int not null);
