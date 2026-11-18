create table users (id integer primary key, age integer, name text not null default 'x', constraint adult check ("users"."age" > 21));
create table orders (id integer primary key, user_id integer references users (id));
