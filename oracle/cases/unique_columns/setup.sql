-- Which sets of a result's columns no two rows agree on, a table's primary key first. A unique index lets NULLs repeat, so a nullable column of one is no such set; a key with another inside it says nothing more.
create table users (email text not null unique, id int primary key, handle text unique, a int not null, b int not null, unique (a, b), unique (a, b, email));
create table composite (x int not null, y int not null, code text not null unique, primary key (x, y));
create table posts (id int primary key, user_id int not null references users (id), slug text not null unique);
insert into users values ('a@x', 1, 'a', 1, 1), ('b@x', 2, null, 1, 2), ('c@x', 3, null, 2, 1);
insert into composite values (1, 1, 'p'), (1, 2, 'q'), (2, 1, 'r');
insert into posts values (1, 1, 's'), (2, 1, 't'), (3, 2, 'u');
