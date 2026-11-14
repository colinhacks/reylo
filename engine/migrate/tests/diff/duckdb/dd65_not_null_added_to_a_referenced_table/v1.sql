create table users (id bigint primary key); create table posts (user_id bigint references users (id));
