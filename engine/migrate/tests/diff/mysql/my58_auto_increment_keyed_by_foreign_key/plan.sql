alter table c
  add column seq int not null auto_increment,
  add key fk (seq);
alter table c add constraint fk foreign key (seq) references p (id);
