create table users (id int primary key, name text); create table posts (id int primary key, user_id int not null, constraint posts_user_fk foreign key (user_id) references users (id));
