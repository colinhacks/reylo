create table users (id int primary key, name text, code text); create view coded as select id, code from users;
