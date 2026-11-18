create table parent (id integer not null auto_increment primary key);
create table child (id integer not null auto_increment primary key, parent_id integer, foreign key (parent_id) references parent (id));
