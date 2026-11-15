create table users (id integer primary key, full_name text not null, n integer not null, code text, n2 integer generated always as (n * 2) stored);
