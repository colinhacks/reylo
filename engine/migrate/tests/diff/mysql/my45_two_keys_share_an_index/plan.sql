alter table child
  drop index parent_id,
  add key fk_to_partition (parent_id);
create table parent_partition_1 (
  id int not null auto_increment,
  primary key (id)
);
alter table child add constraint fk_to_partition foreign key (parent_id) references parent_partition_1 (id);
