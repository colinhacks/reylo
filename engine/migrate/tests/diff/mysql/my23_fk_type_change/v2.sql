create table users (id bigint primary key, name text); create table posts (id int primary key, user_id bigint not null, constraint posts_user_fk foreign key (user_id) references users (id));
