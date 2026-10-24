-- JSON built over rows a derived table or a WITH query reads where it refers to a query around
-- it: the latest few children of each parent, as JSON.
create table person (id integer primary key, first_name text not null, deleted_at text);
create table post (id integer primary key, author_id integer not null references person, title text not null);
create table comment (id integer primary key, post_id integer not null references post, author_id integer not null references person, body text not null, created_at text not null);
