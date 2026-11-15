create table users (id bigint primary key, n integer check (n > 0)); create table posts (uid bigint references Users (ID));
