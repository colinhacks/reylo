create domain code as varchar(8) default 'new' not null check (value in ('abc', 'xyz', 'new'));
create table things (id integer primary key, c code);
