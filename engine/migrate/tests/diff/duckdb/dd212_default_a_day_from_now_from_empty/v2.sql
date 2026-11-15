create table invites (id integer primary key, expires date default (current_date + interval 1 day));
