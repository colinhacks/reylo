-- node references itself, and DuckDB checks such a key against the rows a table held when its transaction began: where a row's key names another row, this copy fails, and the migration with it, changing nothing
create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_node as select * from node;
drop table node;
drop table p;
create table p (id integer, name varchar, primary key (id), check (("name" != '')));
create table node (id integer, p_id integer, up integer, primary key (id), foreign key (p_id) references p (id), foreign key (up) references node (id));
insert into p (id, name) select id, name from temp.main.reylo_copy_p;
insert into node (id, p_id, up) select id, p_id, up from temp.main.reylo_copy_node;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_node;
