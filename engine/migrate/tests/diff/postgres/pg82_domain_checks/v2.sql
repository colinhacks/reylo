create table kept (id integer primary key);
create domain positive_int as integer;
alter domain positive_int add constraint positive_int_check check (value > 0) not valid;
create function is_email(text) returns boolean language sql immutable as $$ select $1 like '%@%' $$;
create domain email as text check (is_email(value));
create table contacts (id integer primary key, address email);
