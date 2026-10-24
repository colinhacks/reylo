-- Under a plain GROUP BY every group holds a row, so an aggregate that is NULL only over no rows or
-- over nothing but NULLs (`sum`, `avg`, `min`, `max`) is as NULL as its argument, and
-- `json_arrayagg` and `json_objectagg` are never NULL; the server flags them nullable all the
-- same. Without a GROUP BY an empty table gives one row of NULLs, and the variance family is NULL
-- over one row (`var_samp`), so neither is claimed. Users without posts, posts without a date or a
-- note, and every edge value are here, so a wrong non-null meets a NULL.
create table users (id int primary key auto_increment, name varchar(40) not null);
create table posts (id int primary key auto_increment, user_id int not null, title varchar(80) not null, published_at datetime, score double, note varchar(20), key (user_id));
insert into users (name) values ('Ada'), ('Grace'), ('Linus');
insert into posts (user_id, title, published_at, score, note) values (1, 'Hello', '2026-01-01 00:00:00', 1.5, 'n'), (1, 'Draft', null, null, null), (2, 'Notes', '2026-02-03 00:00:00', 2, null);
