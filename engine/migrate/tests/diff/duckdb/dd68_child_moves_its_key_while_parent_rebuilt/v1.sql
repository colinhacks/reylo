create table users (id bigint primary key, n integer); create table orders (id bigint, user_id bigint references users (id));
