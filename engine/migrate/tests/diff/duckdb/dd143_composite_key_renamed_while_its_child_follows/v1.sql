create table p (a integer, b integer, n integer, primary key (a, b)); create table c (id integer primary key, pa integer, pb integer, foreign key (pa, pb) references p (a, b));
