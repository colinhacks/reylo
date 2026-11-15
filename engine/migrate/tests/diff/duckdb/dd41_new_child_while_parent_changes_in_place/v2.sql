create table users (id bigint primary key, age bigint); create table posts (id bigint, user_id bigint references users (id));
