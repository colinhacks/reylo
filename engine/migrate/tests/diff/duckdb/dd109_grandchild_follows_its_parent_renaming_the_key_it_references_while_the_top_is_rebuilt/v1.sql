create table users (id integer primary key, name varchar);
     create table posts (id integer primary key, user_id integer references users (id));
     create table comments (id integer primary key, post_id integer references posts (id));
