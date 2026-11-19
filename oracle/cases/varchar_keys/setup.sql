-- A varchar key beside a value that is text for certain, whatever function or expression makes it:
-- `varchar = text` compares the column as it is, so the key holds. Beside a bpchar value the server
-- compares the column as bpchar, whose = ignores trailing blanks, so 'a', 'a ' and 'a  ' match.
-- No collation here is nondeterministic, which a subquery's value could otherwise bring.
create table vkey (id int primary key, email varchar(255) not null unique, slug varchar(80) not null unique);
insert into vkey values (1, 'a', 's'), (2, 'a ', 's ');
create function norm(text) returns text language sql immutable as $$ select lower(btrim($1)) $$;
create table vk (id int primary key, vc varchar(10) unique);
insert into vk values (1, 'a  '), (2, 'a '), (3, 'a');
