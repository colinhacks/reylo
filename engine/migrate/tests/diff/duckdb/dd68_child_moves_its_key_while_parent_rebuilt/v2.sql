create table users (id bigint primary key, n integer check (n > 0)); create table accounts (id bigint primary key); create table orders (id bigint, user_id bigint references accounts (id));
