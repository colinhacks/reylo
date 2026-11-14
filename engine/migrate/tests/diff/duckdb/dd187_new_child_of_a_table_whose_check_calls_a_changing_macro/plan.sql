create temporary table reylo_copy_parent as select * from parent;
drop table parent;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table parent (id integer, code varchar, primary key (id), check ("valid"(code)));
insert into parent (id, code) select id, code from temp.main.reylo_copy_parent;
drop table temp.main.reylo_copy_parent;
create table child (id integer, parent_id integer, primary key (id), foreign key (parent_id) references parent (id));
