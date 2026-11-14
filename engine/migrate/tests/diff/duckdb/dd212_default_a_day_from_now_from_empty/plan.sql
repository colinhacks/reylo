create table invites (id integer, expires date default (current_date + to_days(cast(trunc(cast(1 as DOUBLE)) as INTEGER))), primary key (id));
