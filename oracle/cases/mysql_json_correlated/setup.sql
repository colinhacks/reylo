-- JSON built over rows a derived table, a WITH query or a LATERAL derived table reads where it
-- refers to a query around it: the latest few children of each parent, as JSON.
create table person (id int primary key, first_name varchar(20) not null, deleted_at datetime);
create table post (id int primary key, author_id int not null, title varchar(20) not null);
create table comment (id int primary key, post_id int not null, author_id int not null, body varchar(40) not null, created_at datetime not null);
