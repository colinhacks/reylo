create table users (id integer primary key, name varchar check (name <> ''));
     create table posts (pid integer primary key, user_id integer references users (id));
     create table comments (id integer primary key, post_id integer references posts (pid));
