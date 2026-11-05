-- An extension's `<@` for its own type leaves the built-in the one `text[]` resolves to.
create extension hstore;
create table posts (id int primary key, tags text[] check (tags <@ array['a', 'b']), h hstore);
insert into posts values (1, '{a}', 'x=>1');
