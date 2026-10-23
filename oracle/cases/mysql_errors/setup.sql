-- What `.try()`'s error type is narrowed to on MySQL: the keys, foreign keys, checks and
-- columns a statement writes through, and nothing where it may run a trigger or a stored function.
create table users (id int primary key, email varchar(20) not null unique, age int, constraint adult check (age >= 18));
create table posts (id int primary key, user_id int not null, title varchar(20) not null, constraint posts_user foreign key (user_id) references users (id) on delete cascade);
create table comments (id int primary key, post_id int, constraint comments_post foreign key (post_id) references posts (id) on delete restrict);
create table audited (id int primary key);
create trigger audited_audit before insert on audited for each row set new.id = new.id;
create function tax(n int) returns int deterministic return n * 2;
create view taxed as select id, tax(age) as taxed from users;
create view user_emails as select id, email from users;
insert into users (id, email, age) values (1, 'ada@example.com', 30);
insert into posts (id, user_id, title) values (1, 1, 'hello');
insert into comments (id, post_id) values (1, 1);
