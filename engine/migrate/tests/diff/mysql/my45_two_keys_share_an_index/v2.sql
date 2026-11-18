create table parent (id integer not null auto_increment primary key);
create table parent_partition_1 (id integer not null auto_increment primary key);
create table child (id integer not null auto_increment primary key, parent_id integer, foreign key (parent_id) references parent (id), constraint fk_to_partition foreign key (parent_id) references parent_partition_1 (id));
