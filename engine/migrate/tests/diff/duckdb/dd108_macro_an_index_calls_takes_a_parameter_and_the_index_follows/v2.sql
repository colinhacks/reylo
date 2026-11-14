create macro norm(s, keep) as case when keep then s else lower(s) end; create table t (id integer primary key, email varchar); create index t_norm on t (norm(email, false));
