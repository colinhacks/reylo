create macro norm(s) as lower(s); create table t (id integer primary key, email varchar); create index t_norm on t (norm(email));
