create table users (id int auto_increment primary key, name varchar(50) not null, email varchar(255) unique);
create table one_column (x int not null);
create table at_columns (`@x` int, `@@y` int);
