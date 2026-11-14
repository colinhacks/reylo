create table users (id int primary key); create table posts (id int primary key, user_id int, foreign key (user_id) references users (id)); create table notes (id int primary key);
