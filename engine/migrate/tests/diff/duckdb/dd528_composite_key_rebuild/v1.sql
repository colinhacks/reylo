create table a (x integer, y integer, v varchar, primary key (x, y)); create table b (id integer primary key, ax integer, ay integer, foreign key (ax, ay) references a (x, y));
