create type addr as struct(city varchar); create table t (id integer primary key, a addr, b addr[]);
