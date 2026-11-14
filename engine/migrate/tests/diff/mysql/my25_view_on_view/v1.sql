create table users (id int primary key, name text, n int); create view named as select id, name from users; create view named_ids as select id from named;
