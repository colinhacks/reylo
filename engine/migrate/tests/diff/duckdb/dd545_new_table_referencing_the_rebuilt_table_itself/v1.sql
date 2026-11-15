create table users (id integer primary key, email varchar); create table posts (id integer primary key, user_id integer references users (id));
