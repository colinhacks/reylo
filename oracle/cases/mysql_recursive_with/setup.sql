-- A recursive WITH query's column is never NULL where its anchor's is and its recursive members'
-- are, given that the column is never NULL in the rows they read: the server flags every column
-- of a recursive WITH query nullable, so the engine proves each member under that hypothesis and
-- drops the columns it fails for until the rest hold. Whatever reads the query may still make a
-- column NULL: an outer join, or a later member that writes NULL into a column an earlier one reads.
create table users (id int primary key, email varchar(255) not null unique, name varchar(255) not null, nickname varchar(255), role enum('admin', 'member') not null default 'member');
create table posts (id int primary key, author_id int not null, title varchar(255) not null, score int, created_at datetime not null default current_timestamp, foreign key (author_id) references users (id));
insert into users values (1, 'a@x', 'Ann', null, 'admin'), (2, 'b@x', 'Bob', 'bobby', 'member'), (3, 'c@x', 'Cy', null, 'member');
insert into posts (id, author_id, title, score) values (1, 1, 'one', 3), (2, 1, 'two', null), (3, 2, 'three', 5), (4, 3, 'four', null);
