create table some_table (id integer not null auto_increment primary key, test varchar(10));
create table users (id integer not null auto_increment primary key, name varchar(50) not null, tid integer, foreign key (tid) references some_table (id));
