create table users (id integer primary key, full_name text not null, n integer not null, code text);
create function bump(x integer) returns integer language sql immutable as 'select x + 2';
