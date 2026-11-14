create type status as enum ('open', 'closed'); create macro ok(status) as status; create table pages (id integer primary key, parent_id integer references pages (id), s boolean check (ok(s)));
