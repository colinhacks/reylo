create type addr as struct(city varchar, zip varchar); create table t (id integer primary key, a addr, b addr[]);
