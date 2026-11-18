create table events (id integer primary key, logged_at timestamp default (now() at time zone 'utc'), flagged boolean default (not (1 is distinct from 2)));
create table audit (id integer primary key, logged_at timestamp default (now() at time zone 'utc'));
