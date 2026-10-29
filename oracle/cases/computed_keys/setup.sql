-- A unique key compared with a value the WHERE computes from parameters and constants alone
-- pins the row as a parameter does, where the build types the value and the `=` can only be the
-- built-in one: `lower` and `upper` of a string are text, an integer sum is the wider integer.
-- citext's `=` stands beside the built-in one here, and the comparison of two texts still
-- resolves to the built-in. A call the build does not type proves nothing. A key reaches a view
-- that passes it through, citext's own and a domain's over it included.
create extension citext;
create domain email_address as citext check (value ~ '@');
create table products (id int primary key, sku text not null unique, code bigint unique, title text not null);
create table members (id int primary key, email email_address not null unique, deleted_at timestamptz);
create view active_members as select id, email from members where deleted_at is null;
create table orders (id int primary key, total int not null);
