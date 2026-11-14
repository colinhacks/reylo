create type mood as enum ('sad', 'ok', 'glad'); create table users (id bigint primary key, m mood); create table posts (id bigint, user_id bigint references users (id));
