create table p (a integer, bee integer, n integer check (n > 0), primary key (a, bee)); create table c (id integer primary key, pa integer, pb integer, foreign key (pa, pb) references p (a, bee));
