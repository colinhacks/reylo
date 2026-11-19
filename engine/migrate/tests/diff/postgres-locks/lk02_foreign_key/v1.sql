create table users (id bigint primary key);
create table posts (id bigint primary key, user_id bigint not null);
