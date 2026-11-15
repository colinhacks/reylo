create table users (id bigint primary key, active boolean not null default true); create table posts (user_id bigint references users (id));
