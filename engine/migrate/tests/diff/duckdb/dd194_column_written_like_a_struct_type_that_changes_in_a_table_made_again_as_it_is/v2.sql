create type addr as struct(city varchar, zip varchar); create macro valid(s) as length(s) > 0; create table t (id integer primary key, code varchar check (valid(code)), other struct(city varchar));
