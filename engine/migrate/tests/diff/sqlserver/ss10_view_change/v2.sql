create table t (id int not null primary key, a int not null, b int null);
GO
create view v as select id, a, b from t;
