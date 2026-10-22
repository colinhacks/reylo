-- A scalar subquery with a WITH of its own is typed as the same statement with the WITH outside:
-- each item is prepared over the subquery's FROM, after the WITH queries of every level it stands
-- in, listed as one WITH.
create table users (id int auto_increment primary key, name text not null, email varchar(255) unique);
create table posts (id int auto_increment primary key, user_id int not null, title text not null, body text, foreign key (user_id) references users (id));
