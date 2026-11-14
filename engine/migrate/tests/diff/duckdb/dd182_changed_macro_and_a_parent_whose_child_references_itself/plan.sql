-- c references itself, and DuckDB checks such a key against the rows a table held when its transaction began: where a row's key names another row, this copy fails, and the migration with it, changing nothing
create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
drop macro x;
create macro x (a) as (a >= 0);
create table p (id integer, v integer, primary key (id), check (x(v)));
create table c (id integer, p_id integer, parent integer, primary key (id), foreign key (p_id) references p (id), foreign key (parent) references c (id));
insert into p (id, v) select id, v from temp.main.reylo_copy_p;
insert into c (id, p_id, parent) select id, p_id, parent from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
