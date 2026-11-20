create table t (a int);
create table t_child (b int) inherits (t);
create table u (a int);
create table u_child (b int) inherits (u);
create publication pub for table only t;
create publication alone for table only u, table only u_child;
