create table users (id bigint primary key, age integer); create table posts (id bigint, user_id bigint references users (id));
