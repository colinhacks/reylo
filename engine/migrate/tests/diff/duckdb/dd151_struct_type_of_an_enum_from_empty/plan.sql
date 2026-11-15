create type mood as enum('a', 'b');
create type a_rec as struct(m enum('a', 'b'), n integer);
create table t (id integer, r a_rec, primary key (id));
