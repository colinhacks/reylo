create table users (id integer primary key, age integer);
create table orders (id integer primary key, user_id integer references users (id));
