create table p (a integer, b varchar, n integer check (n > 0), primary key (a, b)); create table c (id integer primary key, a integer, b varchar, foreign key (a, b) references p (a, b));
