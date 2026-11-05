create table users (id int primary key, email varchar(255) not null unique, name varchar(255) not null, nickname varchar(255));
create table posts (id int primary key, author_id int not null, title varchar(255) not null, score int, created_at datetime not null default current_timestamp, foreign key (author_id) references users (id));
