create table log (id int primary key auto_increment, at datetime); create event prune on schedule every 1 day do delete from log where at < now() - interval 30 day;
