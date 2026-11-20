create table p (id int, c int);
create table t (id int, b bigint, c int);
create statistics st on c, b from t;
