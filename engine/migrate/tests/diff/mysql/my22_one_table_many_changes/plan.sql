-- loses data: users.legacy
alter table users
  drop index n_idx,
  drop column legacy,
  modify column full_name varchar(200) not null,
  modify column n int not null default '0',
  add column added date,
  add key full_name_idx (full_name),
  comment='People';
