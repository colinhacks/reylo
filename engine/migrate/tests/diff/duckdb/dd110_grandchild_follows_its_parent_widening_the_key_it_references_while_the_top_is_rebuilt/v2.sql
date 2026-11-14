create table users (id integer primary key, name varchar check (name <> ''));
     create table posts (id bigint primary key, user_id integer references users (id));
     create table comments (id integer primary key, post_id bigint references posts (id));
