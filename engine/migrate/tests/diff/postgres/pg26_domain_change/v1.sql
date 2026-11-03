create domain code as varchar(8) default 'none' check (value <> '');
create table things (id integer primary key, c code);
