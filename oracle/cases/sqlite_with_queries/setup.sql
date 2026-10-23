-- WITH queries, recursive and not, nested in one another. A WITH query's table belongs to the
-- statement, and the engine describes the schema's tables alone.
create table categories (id integer primary key, parent_id integer references categories (id), name text not null, note text);
insert into categories (id, parent_id, name, note) values (1, null, 'root', null), (2, 1, 'child', 'a note'), (3, 2, 'grandchild', null);
