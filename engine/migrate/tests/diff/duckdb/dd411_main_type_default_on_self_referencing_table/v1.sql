create type mood as enum ('a', 'b'); create table pages (id integer primary key, parent integer references pages (id), a varchar default ('a'::mood)::varchar);
