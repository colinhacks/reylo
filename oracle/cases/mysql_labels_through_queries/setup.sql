-- A column that reads an ENUM or a SET column itself through a derived table or a WITH query
-- keeps the column's labels, which the server's prepare names by the derived column's own name;
-- and a UNION whose every side's column is an ENUM column holds the labels of those columns, which
-- the server types as a VARCHAR. A UNION of SET columns, an expression over the column, or a side
-- of another type, leaves text.
create table users (id int primary key, name varchar(255) not null, role enum('admin', 'member') not null default 'member', tags set('a', 'b'), status enum('on', 'off'));
create table posts (id int primary key, author_id int not null, kind enum('draft', 'published') not null, flags set('x', 'y') not null);
create table swapped (a enum('x', 'y') not null, b enum('p', 'q') not null);
insert into users values (1, 'Ann', 'admin', 'a,b', 'on'), (2, 'Bob', 'member', null, null), (3, 'Cy', 'member', '', 'off');
insert into posts values (1, 1, 'draft', 'x'), (2, 2, 'published', 'x,y');
insert into swapped values ('x', 'p'), ('y', 'q');
create table e3 (a enum('p') not null, b enum('q') not null);
create table e4 (a enum('r') not null, b enum('s') not null);
insert into e3 values ('p', 'q');
insert into e4 values ('r', 's');
create table `a\?b` (v enum('x', 'y') not null);
insert into `a\?b` values ('x');
