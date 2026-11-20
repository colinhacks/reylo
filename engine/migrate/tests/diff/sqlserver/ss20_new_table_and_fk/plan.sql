create table sessions (
  id uniqueidentifier not null default (newid()),
  user_id int not null,
  expires datetimeoffset(3) not null,
  primary key clustered (id)
);
alter table sessions add foreign key (user_id) references users (id) on delete cascade;
