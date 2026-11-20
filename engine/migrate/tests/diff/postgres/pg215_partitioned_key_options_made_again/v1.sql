create table t (id int, constraint t_pkey primary key (id) with (fillfactor = 60)) partition by list (id);
create table t_pd partition of t default;
create table u (id int, constraint u_pkey primary key (id)) partition by list (id);
create table u_pd partition of u default;
