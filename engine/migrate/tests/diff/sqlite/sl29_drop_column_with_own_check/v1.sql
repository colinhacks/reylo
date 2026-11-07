create table users (id integer primary key, full_name text not null, code text check (length(code) > 1));
