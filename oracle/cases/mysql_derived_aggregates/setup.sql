-- A scalar subquery that is an ungrouped aggregate over a derived table is one row, as one over a
-- table or a WITH query is: its aggregate reads the derived table's columns, by the derived table's
-- alias or by a column name the derived table selects, so the aggregate is the subquery's own.
create table users (id int auto_increment primary key, name text not null, email varchar(255) unique);
create table posts (id int auto_increment primary key, user_id int not null, title text not null, body text, published_at datetime, foreign key (user_id) references users (id));
