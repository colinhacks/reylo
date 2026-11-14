-- node references itself, and DuckDB checks such a key against the rows a table held when its transaction began: where a row's key names another row, this copy fails, and the migration with it, changing nothing
create temporary table reylo_copy_node as select * from node;
drop table node;
create table node (id bigint, parent bigint, name varchar, primary key (id), foreign key (parent) references node (id), check (("name" != '')));
insert into node (id, parent, name) select id, parent, name from temp.main.reylo_copy_node;
drop table temp.main.reylo_copy_node;
