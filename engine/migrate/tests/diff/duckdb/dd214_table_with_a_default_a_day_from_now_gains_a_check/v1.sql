create table invites (id integer primary key, email varchar, expires date default (current_date + interval 1 day));
