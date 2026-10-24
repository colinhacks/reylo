-- JSON built over rows a derived table, a WITH query or a LATERAL subquery reads where it refers to
-- a query around it: the latest few children of each parent, as JSON.
create table person (id serial primary key, first_name text not null, last_name text not null, email text not null, deleted_at timestamptz);
create table post (id serial primary key, author_id int not null references person, title text not null, body text not null, status text not null, published_at timestamptz, deleted_at timestamptz);
create table comment (id serial primary key, post_id int not null references post, author_id int not null references person, body text not null, created_at timestamptz not null default now());
