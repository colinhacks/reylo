create table invites (id integer primary key, email varchar check (email <> ''), expires date default (current_date + interval 1 day));
