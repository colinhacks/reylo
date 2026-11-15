create type mood as enum ('a', 'b'); create table pages (id integer primary key, parent_id integer references pages (id), n integer default 1 check (row(n)::struct(mood integer) is not null));
