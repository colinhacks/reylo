create table t (id int primary key, a int, b int);
create statistics st on a, b from t;
alter statistics st set statistics 500;
alter statistics st owner to diff_owner;
