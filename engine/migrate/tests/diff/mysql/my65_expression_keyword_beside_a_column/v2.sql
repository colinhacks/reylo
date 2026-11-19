create table t (id int primary key, `AS` int, x varchar(20), key k ((cast(x AS char(30)))));
