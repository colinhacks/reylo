-- An aggregate used as a window function is NULL only where no non-null value reaches its frame.
-- A frame that always holds the current row (no frame clause, or one that starts at or before
-- the current row and ends at or after it) leaves it that row's value, so it follows its
-- argument; any other frame can be empty. A RANGE frame with an offset can be empty too: over a
-- DATETIME that holds a zero date the server finds no row in it, so it proves nothing. The
-- ordering columns hold NULL and the edge values of their types.
create table users (id int primary key auto_increment, name varchar(40) not null);
create table posts (id int primary key auto_increment, user_id int not null, title varchar(80) not null, published_at datetime, score double, weight decimal(10, 2), note varchar(20), key (user_id));
insert into users (name) values ('Ada'), ('Grace');
insert into posts (user_id, title, published_at, score, weight, note) values (1, 'Hello', '2026-01-01 00:00:00', 1.5, 2, 'n'), (1, 'Draft', null, null, null, null), (2, 'Notes', '2026-02-03 00:00:00', 2, 3, null);
